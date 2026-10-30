# Architecture

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

## Principles
1. **Contracts are the product.** Every service is defined by its OpenAPI, event schemas and MCP tool catalogue in [`contracts/`](../contracts/) before it is coded ([ADR-0001](adr/0001-service-boundaries-and-contract-first.md)).
2. **One capability, three doors.** REST, MCP and SDK from one codebase per service.
3. **Own your data.** One schema and role per service; no cross-schema joins; facts travel by API or event.
4. **Evidence first.** Activity events are immutable; everything derived cites them ([ADR-0006](adr/0006-event-sourced-activity-and-ledger.md)).
5. **Policy outside the model.** Models propose; deterministic code and tenant policy decide ([ADR-0005](adr/0005-autonomy-ladder-and-review.md)).
6. **Few deployables, many boundaries.** ≤ 4 processes through Phase 2, ≤ 8 by Phase 5.

## System context

```
 Fee earners ─► Web app / Outlook add-in ─┐
 Outer agents (Claude, plugin) ───────────┤        ┌────────────────────────────────────────┐
 Scripts / other systems (SDK) ───────────┼─ REST/ │                TimeLex                 │
 Desktop agent (Windows) ─────────────────┘  MCP   │  gateway → services → Postgres/storage │
                                                    └───────┬─────────────┬──────────────────┘
                              Microsoft 365 / Google ◄──────┘             │
                              (mail, calendar, meetings)                  ├─► Model provider (tenant's own key)
                              GhostPractice (sanctioned import/export) ◄──┤
                              ClearCut (invoices, VAT, export) ◄──────────┘
```

## Services

Status: ✅ code exists · 🟡 specified (README + contract, no runtime) · ⬜ not yet specified.

| Service | Lang | Responsibility | Owns data | Phase | Status |
|---|---|---|---|---|---|
| [`gateway`](../services/gateway) | Rust | Edge: authN (API keys, OIDC), tenant resolution, rate limits and metering, routing, outward MCP aggregation, OpenAPI aggregation | none (config only) | 1 | ✅ authN, routing to identity, per-firm limits and metering, header-based `tools/call` routing; MCP aggregation and OIDC pending |
| [`identity`](../services/identity) | Rust | Tenants, users, roles, API keys, SSO settings | `identity` | 1 | ✅ library (create firm, issue/revoke key, authenticate) |
| [`vault`](../services/vault) | Rust | BYO keys and OAuth tokens: envelope encryption, write-only API, spend caps, audit | `vault` | 1 | ✅ v0 (`vault.v1.yaml`): sealed schema, envelope encryption, write-only REST API, Anthropic key check, per-firm audit chain. Lease, MCP and KMS/WIF later |
| [`mcp-host`](../services/mcp-host) | Rust | Agent runtime: MCP clients, model loop, workflows, budgets, escalation, audit | `host` | 1–4 | 🟡 |
| [`capture-hub`](../services/capture-hub) | Rust | Connectors (Microsoft Graph, Google), webhook receipt, subscription renewal, normalisation to activity events | `capture` | 2 | 🟡 |
| [`desktop-agent`](../services/desktop-agent) | Rust | Windows agent: foreground window/document, idle time, local privacy filter, pause, signed updates | local only | 2 | 🟡 |
| [`attribution`](../services/attribution) | Rust + Py worker | Segmentation to time blocks; matter attribution cascade; calibration | `attribution` | 3 | 🟡 |
| [`narrative`](../services/narrative) | Rust | Cited narratives, per-client style, number guard | `narrative` | 3 | 🟡 |
| [`time-ledger`](../services/time-ledger) | Rust | Time entries (append-only), units/rates, autonomy policy engine, review queue, audit | `ledger` | 3–4 | 🟡 |
| [`matters`](../services/matters) | Rust | Clients, matters, contacts, conflicts, rate cards | `matters` | 3 (min), 6 | 🟡 |
| [`billing`](../services/billing) | Rust | WIP, pro-forma, fee-note prep; calls ClearCut | `billing` | 5 | 🟡 |
| [`trust`](../services/trust) | Rust | Trust and business ledgers, reconciliations, LPC reports | `trust` | 7 (gated) | 🟡 |
| `docs` | Rust + Py | Document store, search, assembly (human-reviewed) | `docs` | 6 | ⬜ |
| `insights` | Python | Utilisation, realisation, WIP ageing, forecasting | read models | 8 | ⬜ |

Workers: [`workers/py-worker`](../workers/py-worker) (redaction ✅; OCR, embeddings, calibration planned).
Clients: `packages/sdk` (TS stub ✅), web app (Phase 4–5), Outlook add-in (Phase 4), `plugin/` (manifest ✅).
Connectors for other practice systems live under `connectors/` behind a `PracticeSystemAdapter` ([ADR-0009](adr/0009-ghostpractice-interop-strategy.md)).

## Core data flow (time)

```
Graph/Google ─► capture-hub ─┐
desktop-agent ───────────────┤  ActivityEvent (immutable, idempotent id)
manual quick-capture ────────┘          │ outbox → bus
                                         ▼
                           attribution: segment → TimeBlock
                                         │ cascade: keys → history → retrieval → small model → stronger model → human
                                         ▼
                           narrative: template + guarded prose (cites evidence)
                                         ▼
                           time-ledger: policy check ─► post (L3) │ review queue (exceptions)
                                         ▼
                           billing: WIP → pro-forma ─► ClearCut (maths, VAT, export) ─► certify ─► fee note
```

## Contracts
| Location | Purpose | Enforced by |
|---|---|---|
| `contracts/events/*.schema.json` | Event payloads (JSON Schema 2020-12) | `timelex-types` tests validate examples against schemas |
| `contracts/openapi/*.yaml` | REST (OpenAPI 3.1) | Gateway tests; SDK generation (P1-09) |
| `contracts/mcp/` | MCP tool catalogue | Checked against Rust types (P1-08) |
| `contracts/examples/` | Golden examples used by tests and docs | CI |

## Tenancy and isolation
Row-level security by `tenant_id` in every schema, tenant set per request by the platform kit and enforced by Postgres policy; per-tenant keys; per-tenant embeddings namespace and memory store. Upgrade path to schema- or database-per-tenant. See [ADR-0007](adr/0007-tenancy-and-data-residency.md).

## Security architecture (summary; see [THREAT_MODEL.md](THREAT_MODEL.md))
- Vault is the only decryptor; plaintext is in memory only and never logged ([ADR-0004](adr/0004-byo-keys-and-secrets-vault.md)).
- Captured content is untrusted data; extraction workflows have no tools; policy is outside the model.
- Service-to-service identity, least-privilege database roles, signed releases and SBOM (Phase 8), dependency scanning in CI from day one.

## Observability
OpenTelemetry traces and metrics from the platform kit; per-model-call records (model, tier, effort, tokens, cache hits, escalation reason, Rand cost); structured logs with redaction; an audit log distinct from operational logs.

## Deployment
| Mode | Stack | Phase |
|---|---|---|
| Local dev | Docker Compose: Postgres, MinIO (S3-compatible), services, py-worker | 1 |
| Self-host | Compose or Helm; optional NATS JetStream | 5 |
| Hosted | South African cloud region where available; container platform; managed Postgres; KMS | 8 |

## Non-functional targets (to be measured, not promised)
| Area | Target (proposed) |
|---|---|
| Capture-to-proposed-entry latency | median < 5 min after the work ends |
| Gateway overhead | p95 < 50 ms |
| Correctness | Same input, same cents, on every platform; ledgers append-only |
| Availability | Capture queues survive provider outages; no data loss |
| Cost | Model spend per fee earner reported per tenant; ≤ R60/month illustrative target |

## Open questions
Tracked in [TASKS.md](TASKS.md) (`T0-*` research tasks, `P1-08` MCP SDK check).
