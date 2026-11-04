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
| [`gateway`](../services/gateway) | Rust | Edge: authN (API keys, OIDC), tenant resolution, rate limits and metering, routing, outward MCP aggregation, OpenAPI aggregation | none (config only) | 1 | ✅ authN, routing to identity, per-firm limits and metering, header-based `tools/call` routing, the manual quick-capture route and page (P2-08, [ADR-0022](adr/0022-manual-quick-capture.md)), the public privacy-exclusion routes (P2-07, [ADR-0023](adr/0023-privacy-exclusions.md)), and the public data-flow register plus the admin privacy page (P2-10, [ADR-0024](adr/0024-data-flow-register-and-disclosure.md)); MCP aggregation and OIDC pending |
| [`identity`](../services/identity) | Rust | Tenants, users, roles, API keys, SSO settings | `identity` | 1 | ✅ library (create firm, issue/revoke key, authenticate) |
| [`vault`](../services/vault) | Rust | BYO keys and OAuth tokens: envelope encryption, write-only API, spend caps, audit | `vault` | 1 | ✅ v0 (`vault.v1.yaml`, routed by the gateway since 0.4.0): sealed schema, envelope encryption, write-only REST API, Anthropic key check, per-firm audit chain, `secret.*` events through the outbox. The internal lease (`vault.internal.v1.yaml`, [ADR-0020](adr/0020-workload-identity-and-leases.md)) serves a trusted service by purpose and audits the use. MCP and KMS/WIF later |
| [`mcp-host`](../services/mcp-host) | Rust | Agent runtime: MCP clients, model loop, workflows, budgets, escalation, audit | `host` | 1–4 | 🟡 model loop, MCP client and vault lease (P1-13, [ADR-0018](adr/0018-model-loop-money-and-failure-rules.md), [ADR-0020](adr/0020-workload-identity-and-leases.md)): provider trait, Anthropic adapter, integer-Rand budgets, the tenant-conditional Haiku→Sonnet→Opus→Fable ladder, the discovery-only MCP client and a workload-token lease client. Workflows and HTTP routes pending |
| [`capture-hub`](../services/capture-hub) | Rust | Connectors (Microsoft Graph, Google), webhook receipt, subscription renewal, normalisation to activity events | `capture` | 2 | ✅ framework, manual capture and privacy exclusions (P2-02 [ADR-0021](adr/0021-idempotent-capture-and-replay.md), P2-08 [ADR-0022](adr/0022-manual-quick-capture.md), P2-07 [ADR-0023](adr/0023-privacy-exclusions.md)): canonical event id, idempotent ingest with `activity.recorded` in the same transaction, `Connector` trait with atomic page+cursor, replay, `POST /v1/entries` for hand-recorded work, and exclusions plus the metadata-only gate enforced before storage. Real connectors, subscriptions and the redaction worker pending |
| [`desktop-agent`](../services/desktop-agent) | Rust | Windows agent: foreground window/document, idle time, local privacy filter, pause, signed updates | local only | 2 | 🟡 |
| [`attribution`](../services/attribution) | Rust + Py worker | Segmentation to time blocks; matter attribution cascade; calibration | `attribution` | 3 | 🟡 stages 0–1 exist as a pure library (P3-05, [ADR-0028](adr/0028-attribution-stages-zero-one.md)): segmentation in [`timelex-segmentation`](../crates/timelex-segmentation), hard keys and history in [`timelex-attribution`](../crates/timelex-attribution); retrieval, model stages, persistence and routes pending |
| [`narrative`](../services/narrative) | Rust | Cited narratives, per-client style, number guard | `narrative` | 3 | 🟡 |
| [`time-ledger`](../services/time-ledger) | Rust | Time entries (append-only), units/rates, autonomy policy engine, review queue, audit | `ledger` | 3–4 | 🟡 |
| [`matters`](../services/matters) | Rust | Clients, matters, contacts, conflicts, rate cards | `matters` | 3 (min), 6 | ✅ minimal (P3-04, [ADR-0027](adr/0027-matters-natural-keys-and-import.md)): clients and matters under the firm's own keys, canonical unique references, explicit aliases, one rate card per matter, and all-or-nothing CSV import. Contacts, the conflict index and a gateway route pending |
| [`billing`](../services/billing) | Rust | WIP, pro-forma, fee-note prep; calls ClearCut | `billing` | 5 | 🟡 |
| [`trust`](../services/trust) | Rust | Trust and business ledgers, reconciliations, LPC reports | `trust` | 7 (gated) | 🟡 |
| `docs` | Rust + Py | Document store, search, assembly (human-reviewed) | `docs` | 6 | ⬜ |
| `insights` | Python | Utilisation, realisation, WIP ageing, forecasting | read models | 8 | ⬜ |

Workers: [`workers/py-worker`](../workers/py-worker) (redaction ✅; OCR, embeddings, calibration planned).
Evaluation: [`eval/synthetic`](../eval/synthetic) generates the deterministic, labelled synthetic firm ([ADR-0025](adr/0025-synthetic-gold-set.md)); the harness and red-team corpus are still planned (P3-11, P3-10).
Clients: `packages/sdk` (typed client ✅; gateway, identity, vault and activity-event types are generated from `contracts/` by `tools/sdk-gen`), web app (Phase 4–5), Outlook add-in (Phase 4), `plugin/` (manifest ✅).
New services start from [`tools/new-service`](../tools/new-service): it generates a workspace member with the gateway-token check, tenant transactions, `/health`, telemetry and database tests, and patches the workspace members and the Postgres bootstrap ([ADR-0017](adr/0017-new-service-generator.md)).
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

Segmentation is a pure library, [`timelex-segmentation`](../crates/timelex-segmentation): events are grouped by thread, document or counterparty within a five-minute gap, meetings, calls and manual entries claim their time before background work, and the resulting blocks never overlap. The sum of block durations equals the union of the input intervals, and every block cites the events that touch it ([ADR-0026](adr/0026-deterministic-segmentation.md)).

Attribution starts with [`timelex-attribution`](../crates/timelex-attribution), also a pure library: stage 0 finds exact matter references and aliases as whole tokens and resolves client domains to a client, and stage 1 remembers thread/document assignments and participant sets. A decision is either one matter, a review with every honest candidate, or unattributed for later stages. A reference on one event never assigns a counterparty block that mixes matters, a closed matter always needs a person, and history never outranks a hard key. On the synthetic gold set this stage is measured at 100% precision on 102 assigned blocks, with 71 blocks in review and 20 unattributed ([ADR-0028](adr/0028-attribution-stages-zero-one.md)).

## Events and the outbox
A service writes the event envelope into `platform.outbox` in the same transaction as the state change it describes. A relay claims undelivered rows, publishes them at least once, and marks them; consumers dedupe on the envelope id, so a redelivery has one effect. `secret.*` is the first family. Envelopes are JSON Schema contracts mirrored by `timelex-types`, and never carry secret values. See [ADR-0014](adr/0014-transactional-outbox.md); the broker path is [ADR-0010](adr/0010-messaging-and-sync-contracts.md).

## Audit chains
Tenant-scoped audit entries are hash-chained by [`timelex-audit`](../crates/timelex-audit): each entry stores its predecessor's hash and its own SHA-256 over seq, microsecond time, kind, actor, subject and a non-secret detail, with a per-service domain tag. `verify` recomputes a full chain and returns its head; comparing that head with the separately stored anchor (`vault.chain_heads` today) also catches a tail rewritten with recomputed hashes. The vault hashes its secret audit rows through the library, and other services with an audit obligation use the same crate.

## MCP transport
Services that serve MCP mount `timelex_mcp::router`. The adapter owns the only `rmcp` dependency (pinned exactly), requires explicit Origin and Host allow-lists and a non-zero body limit, turns off legacy sessions, requires per-request `_meta`, uses `NeverSessionManager`, and advertises only 2026-07-28. Its contract tests are a Rust port of the P1-08 probe script, so an SDK bump that changes transport behaviour fails CI. Task state, audit and events stay in the owning service's Postgres schema. See [ADR-0015](adr/0015-mcp-adapter.md).

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

[`timelex-telemetry`](../crates/timelex-telemetry) is the workspace's only OpenTelemetry dependency. `instrument(router, service)` gives every HTTP request one server span (named by method and matched route) and two metrics (requests counter, duration histogram), and extracts an incoming W3C `traceparent`; `inject` carries that context on every internal call, so one trace runs from the caller through the gateway into the service. Export is opt-in through `OTEL_EXPORTER_OTLP_ENDPOINT` (OTLP/HTTP protobuf); without it ids and propagation still work. Logs are one `tracing` subscriber in JSON or plain text (`TIMELEX_LOG_FORMAT`) with sensitive field names and bearer tokens redacted before formatting. See [ADR-0016](adr/0016-telemetry-kit.md).

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
