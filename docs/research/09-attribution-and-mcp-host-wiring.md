# 09. Attribution service routes, Postgres schema, and mcp-host wiring

_Written by Kone Tshivhinda & Claude · 2026-11-20_

## Purpose

Document the planned HTTP surface, Postgres schema, and `mcp-host` runtime wiring for `services/attribution` (`timelex-attribution-service`) and `services/mcp-host` (`timelex-mcp-host`) so Phase 2 implementation (`src/main.rs`, SQLx migrations, and `deploy/docker-compose.yml` registration) can proceed without ambiguity.

Both crates are currently library-only (`src/lib.rs`, no `src/main.rs`). The `attribution` Postgres schema and `svc_attribution` role are already provisioned by `deploy/postgres/bootstrap/001_roles_and_schemas.sql` and `004_provision_services.sql`, while the service-owned migrations, HTTP binary, and Compose entries are pending Phase 2.

## 1. `services/attribution` HTTP routes

When `services/attribution` exposes its `axum` HTTP binary (`TIMELEX_ATTRIBUTION_ADDR`, default `0.0.0.0:8085`), it will sit behind `services/gateway` (`TIMELEX_SERVICE_URLS` entry `attribution=http://attribution:8085`, routed via `/v1/attribution/*`).

### Authentication and headers

- **Gateway to service**: `X-Timelex-Principal` carrying a short-lived Ed25519-signed `PrincipalClaims` token (`timelex-auth`, `ServiceAudience::Attribution`), plus W3C `traceparent` propagated by `timelex-telemetry`.
- **Health probe**: `GET /health` is unauthenticated and returns `200 OK` (`"ok"`) for Docker Compose and orchestrator healthchecks.
- **Tenant scoping**: Every `/v1/attribution/*` handler extracts `tenant_id` from verified `PrincipalClaims` and binds `app.current_tenant()` inside a `TenantTx` transaction.

### Endpoints

| Method | Path | Purpose | Request payload | Response payload (`200` / `201`) |
|---|---|---|---|---|
| `GET` | `/health` | Liveness/readiness probe | None | `200 OK`, plain text `ok` |
| `POST` | `/v1/attribution/run` | Run stages 0-3 over one or more `ActivityBlock` items for the tenant | `{ "block_ids": ["uuid"], "blocks": [ActivityBlock], "dry_run": false }` | `200 OK`: `{ "run_id": "uuid", "outcomes": [AttributionOutcomeDto] }` |
| `GET` | `/v1/attribution/runs/{id}` | Fetch a single attribution run's status, stage breakdown, cost, and decisions | Path `id: uuid` | `200 OK`: `{ "run_id": "uuid", "status": "completed|blocked|needs_review", "cost_cents": number, "outcomes": [...] }` |
| `GET` | `/v1/attribution/queue` | List blocks routed to `NeedsReview` (abstained or tied) awaiting human decision | Query `?limit=50&cursor=...&status=pending` | `200 OK`: `{ "items": [QueueItem], "next_cursor": "..." }` |
| `POST` | `/v1/attribution/queue/{id}/decide` | Record a fee-earner's manual attribution or dismissal for a queued block | `{ "action": "attribute|dismiss", "matter_id": "uuid?", "reason": "string?" }` | `200 OK`: `{ "decision_id": "uuid", "status": "decided" }` |

### Error codes

- `400 Bad Request`: Malformed JSON, empty `blocks` batch, or `action=attribute` without a valid `matter_id`.
- `401 Unauthorized`: Missing, expired, or invalid `X-Timelex-Principal` token.
- `403 Forbidden`: Token audience mismatch or cross-tenant resource access.
- `404 Not Found`: Unknown `run_id`, `queue_item_id`, or referenced `matter_id` for the tenant.
- `422 Unprocessable Entity`: Terminal `Blocker` surfaced during model stages (`KeyRejected`, `NoRungAvailable`, `SpendCapExceeded`) when synchronous run fails policy checks (otherwise persisted on the run as `blocked`).
- `503 Service Unavailable`: `ProviderUnavailable` (upstream 408/429/5xx) or Postgres pool exhaustion.

## 2. Postgres schema (`attribution`)

### Bootstrap status

`deploy/postgres/bootstrap/001_roles_and_schemas.sql` and `004_provision_services.sql` already create:

- Role `svc_attribution` (`LOGIN NOBYPASSRLS`) and schema `attribution` owned by `timelex_migrator`.
- Default privileges on `attribution` tables and sequences for `svc_attribution`.
- Execute grants on `app.current_tenant()` and `app.assert_tenant()`, plus `SELECT, INSERT` on `app.outbox`.

### Planned migrations (`services/attribution/migrations/0001_attribution.sql`)

Every table MUST include `tenant_id uuid NOT NULL DEFAULT app.current_tenant()` and enable `FORCE ROW LEVEL SECURITY` with a `tenant_id = app.current_tenant()` policy, matching `services/matters/migrations/0001_matters.sql`.

1. **`attribution.attribution_runs`**
   - `id uuid PRIMARY KEY DEFAULT gen_random_uuid()`
   - `tenant_id uuid NOT NULL DEFAULT app.current_tenant()`
   - `status text NOT NULL` (`'running' | 'completed' | 'needs_review' | 'blocked'`)
   - `blocker text` (nullable: `'key_rejected' | 'no_rung_available' | 'provider_unavailable' | 'spend_cap_exceeded'`)
   - `total_blocks integer NOT NULL DEFAULT 0`
   - `cost_cents double precision NOT NULL DEFAULT 0`
   - `created_at timestamptz NOT NULL DEFAULT now()`
   - `completed_at timestamptz`
   - Index: `(tenant_id, created_at DESC)`

2. **`attribution.matter_embeddings`**
   - `matter_id uuid NOT NULL`
   - `tenant_id uuid NOT NULL DEFAULT app.current_tenant()`
   - `dimensions integer NOT NULL DEFAULT 128`
   - `vector real[] NOT NULL` (deterministic `HashEmbedder` projection from `timelex-retrieval`)
   - `document_hash text NOT NULL` (to skip redundant re-embedding)
   - `updated_at timestamptz NOT NULL DEFAULT now()`
   - Primary key: `(tenant_id, matter_id)`
   - Index: `(tenant_id, updated_at DESC)`

3. **`attribution.attribution_decisions`**
   - `id uuid PRIMARY KEY DEFAULT gen_random_uuid()`
   - `tenant_id uuid NOT NULL DEFAULT app.current_tenant()`
   - `run_id uuid REFERENCES attribution.attribution_runs(id) ON DELETE SET NULL`
   - `block_id uuid NOT NULL`
   - `matter_id uuid` (nullable when `outcome = 'needs_review'` or `'dismissed'`)
   - `stage text NOT NULL` (`'stage0_rule' | 'stage1_alias' | 'stage2_cheap_model' | 'stage3_strong_model' | 'human_review'`)
   - `outcome text NOT NULL` (`'attributed' | 'needs_review' | 'dismissed'`)
   - `confidence real` (nullable, constrained `confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)`)
   - `reason jsonb NOT NULL` (serialized `RuleReason` + optional `RungFailure` list)
   - `decided_by uuid` (nullable user id for manual queue decisions)
   - `created_at timestamptz NOT NULL DEFAULT now()`
   - `decided_at timestamptz`
   - Unique index: `(tenant_id, block_id)` (one active decision per block)
   - Partial index for queue polling: `(tenant_id, created_at ASC) WHERE outcome = 'needs_review'`

## 3. `mcp-host` wiring requirements

`services/mcp-host` (`timelex-mcp-host`) is the model execution and key-leasing layer consumed by `services/attribution` (and future LLM-backed services).

### Vault key lease integration

- **Endpoint**: `POST {TIMELEX_VAULT_ADDR}/internal/secrets/anthropic_api_key/lease` (defined in `contracts/openapi/vault.internal.v1.yaml`).
- **Auth**: `X-Timelex-Principal` header signed by the caller service's Ed25519 key (`PrincipalClaims::new(tenant_id, caller_service, ServiceAudience::Vault)`).
- **Request body**: `{ "purpose": "attribution" }`.
- **Response**: `{ "value": "<secret>", "last_four": "<4-chars>" }` wrapped immediately in `timelex_vault::SecretValue` (zeroized on drop; never logged or returned).
- **Error mapping** (`VaultError`): `404 -> NotConfigured`, `401/403 -> Unauthorized`, other/network -> `Unavailable` (10s HTTP timeout).

### Anthropic provider transport

- **Endpoint**: `POST {ANTHROPIC_BASE_URL}/v1/messages` (defaults to `https://api.anthropic.com`).
- **Headers**: `x-api-key` (leased `SecretValue`), `anthropic-version: 2023-06-01`, `content-type: application/json`, user-agent `timelex-mcp-host/0.1`.
- **Timeout**: 30s per request; no built-in retry loop (retries and backoff belong to the caller workflow against the same `SpendCap`).
- **Model tiers** (`ModelCatalog`): `ModelTier::Cheap` (`claude-haiku-4-5-20251001`) and `ModelTier::Strong` (`claude-sonnet-4-6`).
- **Error mapping** (`ModelError`): `401 -> KeyRejected` (stop), `400/403/404 -> ModelUnavailable` (try next rung), `408/429/5xx/network -> Unreachable` (stop), malformed/empty reply -> `Unusable`.

### Environment variables and Compose wiring

When `services/attribution` and `services/mcp-host` are wired into `deploy/docker-compose.yml`:

- `TIMELEX_ATTRIBUTION_ADDR`: bind address for `timelex-attribution-service` (e.g. `0.0.0.0:8085`).
- `TIMELEX_ATTRIBUTION_DATABASE_URL`: `postgres://svc_attribution@postgres:5432/timelex`.
- `TIMELEX_VAULT_ADDR`: internal URL of `services/vault` (e.g. `http://vault:8082`) passed to `VaultConfig::new`.
- `TIMELEX_CALLER_SERVICE`: service identifier for `PrincipalClaims` (e.g. `attribution`).
- `TIMELEX_SERVICE_KEYS_DIR` / `/keys:ro` volume mount: provides the Ed25519 signing key for `PrincipalSigner` so `VaultLeaseClient` can mint `ServiceAudience::Vault` tokens.
- `TIMELEX_SPEND_CAP_CENTS`: optional per-run/per-tenant budget cap for `SpendCap` (`CostTracker` tracks Haiku vs Sonnet millicent costs).
- `ANTHROPIC_BASE_URL`: optional override for `AnthropicProvider::new` (defaults to `https://api.anthropic.com`).
- `gateway` `TIMELEX_SERVICE_URLS`: add `attribution=http://attribution:8085` and route `/v1/attribution/*` in `services/gateway/src/router.rs`.
