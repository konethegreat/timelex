# Tasks

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

Legend: ✅ done · 🔄 in progress · ⬜ not started. `P#-##` build tasks; `T#-*` research, legal and validation tasks; `G#-##` gates. Dependencies in the "After" column. Every task is expected to produce: code + tests (or a decision record), and an update to this file and [WORKLOG.md](WORKLOG.md).

## Immediate next five
1. **T0-R01** GhostPractice demo or trial, plus three user interviews. _Kit ready:_ [interview-kit](validation/interview-kit.md), [demo-checklist](validation/demo-checklist.md).
2. **T0-B01** Five fee-earner interviews: how time is recorded today and what they would let a machine post.
3. **P1-01 → P1-03** Postgres schemas and roles, tenancy with row-level security, identity.
4. ~~P1-08~~ Server side verified 20 Oct 2026 ([research 08](research/08-mcp-rust-sdk-spike.md)); ~~P1-15~~ adapter crate with contract tests done 1 Nov 2026 ([ADR-0015](adr/0015-mcp-adapter.md)). Next: wire the services to the adapter as their MCP tools land; **T0-L05** send Anthropic the written questions.
5. **P3-01** Start the synthetic gold-set generator now; it is the long pole for Gate A.

## Phase 0: Foundations
| ID | Task | After | Status |
|---|---|---|---|
| P0-01 | Research files 01–06 | | ✅ |
| P0-02 | ADR-0001…0010, vision, architecture, autonomy, threat model | P0-01 | ✅ |
| P0-03 | Monorepo scaffold, CI (Rust, Python, TypeScript), licence, community files | | ✅ |
| P0-04 | `timelex-units`: billing units, rounding rules, value in cents (tested) | | ✅ |
| P0-05 | Contracts v0: activity-event schema, example, gateway OpenAPI, MCP catalogue draft | P0-02 | ✅ |
| P0-06 | `timelex-types` contract types with schema-validation test | P0-05 | ✅ |
| P0-07 | `services/gateway` health and meta endpoints | | ✅ |
| P0-08 | `workers/py-worker`: redaction module (SA ID, phone, email, card) with tests | | ✅ |
| P0-09 | `packages/sdk` TypeScript stub, Claude plugin manifest | | ✅ |
| P0-10 | Public GitHub repo, topics, templates, CodeQL, Dependabot, Scorecard, labels | P0-03 | 🔄 |

## Validation, research and legal (T0 to T5)
| ID | Task | Owner | Status |
|---|---|---|---|
| T0-R01 | GhostPractice demo/trial; interview 3 users; capture export/import options | Kone | ⬜ kit ready (claim register in [research 01 §8](research/01-ghostpractice-teardown.md)) |
| T0-R02 | Teardown of SA competitors (LegalSuite, Legal Interact, LawPracticeZA, others) | Kone + Claude | ⬜ |
| T0-B01 | 5 fee-earner interviews; willingness to pay in Rand | Kone | ⬜ kit ready |
| T0-B02 | Trademark and name check for "TimeLex" | Kone | ⬜ |
| T0-L01 | SA attorney and data-protection lawyer review of research 03 | Kone | ⬜ question list in research 03 §1.3, §2 |
| T0-L02 | Anthropic: retention/ZDR/residency and key-custody pattern in writing | Kone | 🔄 public terms read ([research 07](research/07-anthropic-terms-and-key-custody.md)); **written confirmation open** |
| T0-L03 | Read full LPC Part XII (rules 54–57) from the primary text; list software duties | Kone + auditor | 🔄 rules 54.1, 54.6–54.15, 54.19, 54.20, 54.24, 54.34, 57 read in a republication; duties listed; **Gazette text and unread rules remain** |
| T0-L04 | Check Information Regulator for the final transborder-transfer guidance | Kone | ✅ not published as of 20 Oct 2026, announced as planned; re-check quarterly |
| T0-L05 | Send Anthropic the written questions in research 07 §7.3; file the reply | Kone | ⬜ |
| T0-L06 | Read Gazette 41781 (Notice 401 of 2018) and the amendments; read rules not yet read | Kone + auditor | ⬜ |
| T0-A01 | Decide: own agent loop vs Managed Agents / Agent SDK (spike) | Claude | ⬜ (due P4) |
| T0-A02 | Can provider-reported cost be reconciled with our metering? | Claude | 🔄 Anthropic documents a Usage and Cost API by workspace; untested |
| T0-A03 | Spike Workload Identity Federation with a TimeLex-run issuer (only after T0-L05 answer) | Claude | ⬜ blocked on T0-L05 |
| T2-R01 | Research Google Workspace capture (Gmail push, Calendar watch, Meet) | Claude | ⬜ |
| T2-R03 | Spike: Rust + Windows UI Automation for foreground window/document title; **include remote-desktop and virtual-workstation sessions** | Claude | ⬜ |
| T5-L01 | Review GhostPractice licence terms on exports, imports and automation (no public terms found; "Terms of Use" link points to a PAIA file) | Kone | ⬜ |

## Phase 1: Platform kit
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P1-01 | Postgres + `sqlx` migrations; one schema and role per service | | Role cannot read other schemas (test) | ✅ ([ADR-0011](adr/0011-tenancy-enforcement-and-database-roles.md); 7 tests) |
| P1-02 | Tenancy middleware + row-level security | P1-01 | Cross-tenant read fails in property test | ✅ `TenantPool`/`TenantTx` + 11 tests incl. property tests; gateway-side tenant claim is P1-04 |
| P1-03 | `identity`: tenants, users, roles, API keys | P1-02 | Create tenant, issue key, authenticate | ✅ library: [contract](../contracts/openapi/identity.v1.yaml), domain crate (58 tests), schema, store with 16 database tests (create firm, issue, revoke, authenticate). HTTP layer waits for P1-04; users/roles/SSO and idempotency keys are later tasks |
| P1-04 | `gateway`: authN, rate limits, metering, header-based routing. **Part a ✅**: authN through `identity.authenticate`, internal token ([`timelex-auth`](../crates/timelex-auth), 20 tests), identity HTTP layer (11 tests), gateway routing (13 stub tests, 5 end-to-end tests); [ADR-0012](adr/0012-internal-service-authentication.md). **Part b ✅**: contracts (gateway 0.3.0, `contracts/mcp/routes.json`, [ADR-0013](adr/0013-gateway-limits-and-metering.md)); per-firm rate limits (429 + `Retry-After`), metadata-only metering, loopback `/internal/usage`, and `POST /mcp` routed by `MCP-Protocol-Version`/`Mcp-Method`/`Mcp-Name` without parsing the body (25 unit, 2 contract, 24 stub, 6 end-to-end tests). Outward MCP aggregation (`tools/list`, SSE) is P1-15 | P1-03 | Authenticated call routes to a service | ✅ part a and b |
| P1-05 | `vault` v0: envelope encryption, write-only API, key test, audit. Stored-key mode only; WIF waits on T0-L05 ([ADR-0004 proposal](adr/0004-byo-keys-and-secrets-vault.md)) | P1-02 | Secret never returned, never logged (test) | ✅ contract 0.1.0 ([`vault.v1.yaml`](../contracts/openapi/vault.v1.yaml)); domain crate (27 unit), sealed schema, store (16 database tests), HTTP layer with internal `principal` tokens (13 HTTP tests), `anthropic_api_key` checked against the real provider; gateway 0.4.0 routes the five operations with a `vault`-audience token (27 stub, 9 end-to-end tests). The internal lease landed with its first caller in P1-13 ([`vault.internal.v1.yaml`](../contracts/openapi/vault.internal.v1.yaml), [ADR-0020](adr/0020-workload-identity-and-leases.md)); events landed in P1-06 |
| P1-06 | Outbox + relay (Postgres) | P1-01 | Event published exactly-once-effect with idempotent consumer | ✅ [ADR-0014](adr/0014-transactional-outbox.md); [`secret-event.schema.json`](../contracts/events/secret-event.schema.json) mirrored in `timelex-types` (7 tests); [`timelex-outbox`](../crates/timelex-outbox) — append in the writer's transaction, `SKIP LOCKED` claim with a lease, backoff, `Publisher` (2 unit, 4 database tests incl. redelivery); vault queues one `secret.*` event per write in the same transaction, never the value (12 database tests); relay runs in the vault process; gateway end-to-end asserts the five lifecycle events |
| P1-07 | `docker compose` dev stack (Postgres, S3-compatible storage, services, py-worker) | P1-01 | `docker compose up` healthy | ✅ [`Dockerfile`](../Dockerfile) builds one image with the gateway, identity, vault and capture binaries; [`deploy/docker-compose.yml`](../deploy/docker-compose.yml) runs Postgres 17 (a one-shot `bootstrap` container re-applies roles, schemas, tenancy helpers and dev logins on every start, so a newly provisioned service reaches an existing volume), SeaweedFS for S3, a one-shot `keys` container that generates the gateway key pair, operator key and vault master key into a volume, one-shot migrations, and the services with healthchecks. Verified end to end: all services healthy, `/health` and `/v1/meta` through the gateway, a tenant created with the generated operator key, and a `generic_api_key` written, listed and revoked through the gateway (201, metadata only with matching `lastFour`, list 1 then 0, revoke 204) with the vault relay logging `secret.created` and `secret.revoked` and no value in any log. Host ports are 55433 (Postgres) and 9100 (S3) because developer machines commonly hold 5432 and 9000. MinIO's published images no longer resolve (checked 1 Nov 2026), so the stack uses SeaweedFS on the S3 gateway port. `workers/py-worker` is still a library with no daemon, so no worker container yet; it arrives with the first queue consumer (P2-07). The `capture` service joined the stack in P2-02 |
| P1-08 | Verify Rust MCP SDK vs final 2026-07-28 spec; else thin Streamable-HTTP adapter | | `tools/list`/`tools/call` over stateless HTTP | ✅ server side (`rmcp` 3.6.0, [research 08](research/08-mcp-rust-sdk-spike.md)); client side landed in P1-13 |
| P1-09 | OpenAPI → TypeScript SDK generation; contract tests in CI | P0-05 | Drift fails CI | ✅ [`tools/sdk-gen`](../tools/sdk-gen) generates `packages/sdk/src/generated/{gateway,identity,vault}.ts` from `contracts/openapi/*.v1.yaml` and `activity-event.ts` from the event JSON Schema. The generator is a separate workspace pinned to `openapi-typescript` 7.13.0, `json-schema-to-typescript` 16.0.0 and TypeScript 5.9.3, because `openapi-typescript` needs the classic compiler API that TypeScript 7 (the SDK's compiler) no longer ships. `Health`, `Meta` and `ActivityEvent` are now the generated types, and the SDK exports `GatewayPaths`, `IdentityPaths`, `VaultPaths` with their operations and components. `npm run check:sdk` regenerates in memory and exits 1 on a changed, missing or stale file; the TypeScript CI job runs it. Verified by editing a generated file and by adding a stale one (both exit 1) |
| P1-10 | Telemetry kit (OpenTelemetry): traces, metrics, redacted logs | | Trace spans across gateway→service | ✅ [`timelex-telemetry`](../crates/timelex-telemetry) is the workspace's only OpenTelemetry dependency. `init(service)` installs one subscriber (traces, metrics, JSON or plain redacted logs), `instrument(router, service)` gives each request a server span named by method and matched route plus request and duration metrics, and the gateway injects W3C `traceparent` on `authenticate`, `forward` and `relay`; identity and the vault extract it in their own middleware. Export is opt-in through `OTEL_EXPORTER_OTLP_ENDPOINT` (OTLP/HTTP protobuf); without it ids and propagation still work. 9 crate tests (exported span attributes and parentage, metrics, 3 log formats, 4 redaction) and a gateway test over the stub identity: one caller trace contains the gateway span and the service's child span, and both internal calls carry the caller's trace id. Verified at runtime: the gateway binary started with `OTEL_EXPORTER_OTLP_ENDPOINT` set and `TIMELEX_LOG_FORMAT=json` served `/health`, and the rebuilt compose stack came up with all five containers healthy, a tenant created and a `generic_api_key` written and listed through the gateway, the relay emitting `secret.created`, and no value in any log. [ADR-0016](adr/0016-telemetry-kit.md) |
| P1-11 | `tools/new-service` generator | P1-04, P1-10 | New service builds with auth, tenancy, health, CI | ✅ [`tools/new-service`](../tools/new-service) is a dependency-free workspace binary with embedded templates. `generate` checks the name against the rule `platform.provision_service` enforces, refuses an existing service directory, writes nine files (crate manifest, library, binary, HTTP layer with the gateway-token extractor, tenant-scoped store, errors, migration with RLS and grants, database tests, README) and patches exactly two files: the root `members` list and `004_provision_services.sql`, both idempotently. 8 generator tests write into a temporary workspace. Verified by generating `example`: `cargo fmt --check`, `cargo clippy -D warnings` and 5 database tests (health, 401 and 403, create and list, cross-firm isolation, bad input) pass. The Rust CI job now generates and checks the same scratch member, then restores the two patched files. [ADR-0017](adr/0017-new-service-generator.md) |
| P1-12 | Branch protection / rulesets; required checks | P0-10 | PRs need green CI | ⬜ |
| P1-13 | `mcp-host` skeleton: `ModelProvider` trait, Anthropic adapter, budgets; MCP client side tested here; **escalation ladder is tenant-conditional (Fable-class models need a 30-day-retention workspace, [research 07](research/07-anthropic-terms-and-key-custody.md) §3)** | P1-05 | Mock-provider test; Rand cap aborts a loop; a model the key cannot use degrades to the next rung or `needs_review` | 🟡 model loop and MCP client landed ([ADR-0018](adr/0018-model-loop-money-and-failure-rules.md), [ADR-0019](adr/0019-mcp-client-rules.md)): [`services/mcp-host`](../services/mcp-host) has the `ModelProvider` trait with four next-action errors, the Anthropic adapter (key in a header, no provider words in any error), integer micro-rand prices from research 04 §7 at R18/USD with cache multipliers, and a ladder where Fable needs both the tenant's retention flag and a configured price. 2 unit, 14 loop (mock provider) and 9 adapter (local stub) tests, plus a doctest: a R0.005 cap aborts after a R0.0072 call with no second call, and a 404/403 for a model degrades to the next rung. [`timelex-mcp`](../crates/timelex-mcp) now has the client: validated targets (HTTPS except loopback, no URL credentials), discovery-only 2026-07-28 with no legacy fallback, no session recovery, a redacted zeroizing bearer, and closed errors; 9 client tests against the in-repo stateless server and legacy/older stubs, alongside the 23 server probes. The key path landed with [ADR-0020](adr/0020-workload-identity-and-leases.md): [`timelex-auth`](../crates/timelex-auth) signs and verifies workload tokens, the vault leases by purpose (`model_run` → `anthropic_api_key`) on `/internal/v1/...` with a `used` audit entry, and `VaultKeySource` signs, leases and maps every failure to one closed class (5 lease-client tests). Remaining: the `host` schema and the HTTP workflow routes (P3-06) |
| P1-14 | Hash-chained audit-log library | P1-01 | Tamper test detects edit | ✅ [`timelex-audit`](../crates/timelex-audit): domain-separated SHA-256 chain over seq, time, kind, actor, subject and detail; `verify` recomputes a chain and returns its head for comparison with an independently stored anchor; 12 tests cover edits, drops, reordering, forged links, rehashed tails and the vault's exact v0 hash bytes. The vault now hashes through the library, and its database tests verify every stored row |
| P1-15 | `timelex-mcp` adapter: safe `rmcp` config (stateless, Origin checks, required metadata, 2026-07-28 only), exact version pin, the spike's 18 probes as Rust contract tests | P1-08 | Probes pass in CI; an `rmcp` bump that breaks one fails CI | ✅ [ADR-0015](adr/0015-mcp-adapter.md); [`timelex-mcp`](../crates/timelex-mcp) pins `rmcp = "=3.6.0"`, validates explicit origins/hosts and a non-zero body limit, disables legacy sessions, requires per-request metadata, uses `NeverSessionManager`, advertises only 2026-07-28, and repeats all 18 probes plus strict metadata and config checks as 23 Rust contract tests. Gateway aggregation (`tools/list`, SSE, task-id routing) remains open |

## Phase 2: Capture
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P2-01 | Activity-event contract v1 (+ versioning rules) | P0-05 | Backward-compat test | ✅ [`contracts/events/README.md`](../contracts/events/README.md) states the v1 rules: optional additions only, readers ignore unknown fields at every level, and required fields, meanings, types and enum values are fixed within a major. Both event schemas now permit unknown properties, and the Rust types no longer deny them. `timelex-types` (17 tests) validates the frozen 1.0 examples, reads a 1.1 event with unknown fields at the top level and in nested objects, and still rejects a missing required field, a bad enum value and a wrong major. The SDK types are regenerated; the released examples are frozen as `examples/*.v1.0.json` |
| P2-02 | `capture-hub` framework: connector trait, idempotent ingest, replay | P1-06, P2-01 | Duplicate delivery yields one event | ✅ [`services/capture-hub`](../services/capture-hub) with [`capture.v1.yaml`](../contracts/openapi/capture.v1.yaml) and [ADR-0021](adr/0021-idempotent-capture-and-replay.md). An event's id is the first 16 bytes of a length-prefixed SHA-256 over firm, source system, source id and whole-second start; [`timelex-types`](../crates/timelex-types) computes it, pins a golden digest and validates every `1.x` version. `POST /v1/events` recomputes the id (mismatch 400), inserts with `ON CONFLICT (tenant_id, id) DO NOTHING` and appends `activity.recorded` only on the first insert, so the acceptance test stores one row and one outbox event from two deliveries (201 then 200, `created: false`) and fails if either rule is broken. Events are write-once: `svc_capture` has no `UPDATE`/`DELETE` and the tests assert both are refused. The `Connector` trait returns a page plus an opaque cursor; the runner stores the page and advances the cursor in one transaction, refuses an oversized page or a stuck cursor, keeps the last good cursor on a connector failure, and replays from the start without touching the stored cursor. 27 type tests, 14 store, 9 runner and 13 HTTP tests. Compose now runs the capture migrations and service (private interface; gateway routing arrives with P2-08) |
| P2-03 | Microsoft 365 mail connector: admin consent, subscriptions, renewal, lifecycle | P2-02, P1-05 | 14 days, zero missed renewals on a test mailbox | ⬜ |
| P2-04 | Calendar and Teams meetings (attendance; transcripts where permitted) | P2-03 | Meeting durations match Teams reports | ⬜ |
| P2-05 | Google Workspace connector | T2-R01 | Mail+calendar parity with M365 | ⬜ |
| P2-06 | Windows desktop agent v0 (local filter, pause, signed build) | T2-R03 | Foreground doc titles captured; pause honoured | ⬜ |
| P2-07 | Privacy: exclusions (client/domain/matter), redaction pipeline, metadata-only default | P0-08 | Redaction test suite; excluded domains never stored | 🟡 [ADR-0023](adr/0023-privacy-exclusions.md). Exclusions are tenant data in `capture.exclusions` (migration 0002) and are enforced inside the write transaction in [`capture-hub`](../services/capture-hub): an event whose participant is at an excluded domain is never stored and never published, while a connector page still commits its cursor so one rule cannot stall capture. Domains are canonicalised and cover subdomains; client and matter exclusions are recorded now and enforced when attribution exists (P3). A single excluded ingest answers 200 `created:false`, indistinguishable from a duplicate, so the policy does not leak. `settings:manage` manages the list at private `/v1/exclusions` and public `/v1/privacy/exclusions`; adds are idempotent and first-reason-wins. The metadata-only gate refuses `contentRef` without captured content, capture without redaction, redaction without capture and a missing reference. The P0-08 redaction module and its 19 tests exist; the worker queue and container are still pending, so P2-07 stays partial. 8 exclusion-unit, 23 store and 24 HTTP tests in capture-hub; 26 unit, 2 contract, 31 routing and 9 end-to-end in the gateway |
| P2-08 | Manual quick-capture API (+ mobile web) | P2-02 | Entry becomes an activity event | ✅ [ADR-0022](adr/0022-manual-quick-capture.md). [`capture-hub`](../services/capture-hub) serves `POST /v1/entries`: `{entryId, kind, startedAt, endedAt, description}` becomes a canonical activity event (source `manual`, `sourceId` = the `entryId` UUID, kind `call`/`meeting`/`manual_entry`, description whitespace-collapsed into `subject.title` at most 500 characters, `contentCaptured: false`). `capture:write` now belongs to `fee_earner` and `admin` as well as `agent`, so the person who did the work can submit it. The `entryId` is the idempotency key, so a retried submit returns 200 with the same event instead of a second one, and every check runs before storage: end before start, unknown kind or field, and an oversized description are 400 with no row written. [`gateway`](../services/gateway) exposes the public `POST /v1/capture/entries` (contract 0.5.0) and serves a self-contained mobile page at `/capture` with a strict CSP, `nosniff`, `no-referrer` and `no-store`; the key lives in page memory only and a failed save reuses the same `entryId`. 9 manual-normalisation, 20 HTTP, 9 runner and 14 store tests in capture-hub; 26 unit, 2 contract, 29 routing and 9 end-to-end tests in the gateway. Compose routes the public path to the private capture service |
| P2-09 | Gap detector (calendar/activity vs posted time) | P2-04 | Flags a seeded gap | ⬜ |
| P2-10 | Consent and disclosure UX for admins and users | P2-07 | Data-flow register export | 🟡 [ADR-0024](adr/0024-data-flow-register-and-disclosure.md). [`contracts/privacy/data-flow.v1.json`](../contracts/privacy/data-flow.v1.json) is a versioned technical register of eight data categories, eight flows and five processors, each flow and processor marked `active`, `development_only` or `not_active`; it states the metadata-only and exclusion defaults, that exclusions do not clean the past, and that no retention is configured, and disclaims legal advice. [`gateway`](../services/gateway) serves it publicly at `GET /v1/privacy/data-flow` with `no-store` (contract 0.7.0) and a self-contained admin page at `/privacy` that renders the register and lists, adds and removes exclusions; the key stays in page memory, with no web storage, cookie or external asset. 2 new gateway unit tests (28 unit, 2 contract, 31 routing, 9 end-to-end). Still open: a signed per-user consent artefact and a per-firm layer for processor and region choices |
| P2-11 | Backfill as MCP Tasks | P1-08, P2-03 | Resume after restart | ⬜ |

## Phase 3: Understand (Gate A)
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P3-01 | Synthetic gold-set generator (`eval/`): firm, clients, matters, threads, calendars, labels | | Deterministic seed; public | ⬜ |
| P3-02 | Consented real gold-set protocol (POPIA review, anonymisation, storage) | T0-L01 | Written protocol approved | ⬜ |
| P3-03 | Segmentation to time blocks (deterministic, Rust) | P2-01 | Overlap/gap property tests | ⬜ |
| P3-04 | `matters` minimal: clients, matters, aliases, rate cards, import | P1-02 | Import from CSV | ⬜ |
| P3-05 | Attribution stages 0–1: hard keys, history | P3-03, P3-04 | Precision measured | ⬜ |
| P3-06 | Retrieval stage (py-worker embeddings, per-tenant namespace) | P3-05 | Namespace isolation test | ⬜ |
| P3-07 | Model stages (Haiku→Sonnet→Opus) with abstain and failure report | P1-13, P3-06 | Terminal state guaranteed | ⬜ |
| P3-08 | Calibration and per-tenant threshold derivation | P3-01 | Reliability diagram in eval report | ⬜ |
| P3-09 | `narrative`: templates, per-client style, number guard; compare with ClearCut and extract shared library if equal | P3-07 | 100% guard coverage | ⬜ |
| P3-10 | Prompt-injection red-team corpus and tests | P3-07 | Corpus passes in CI | ⬜ |
| P3-11 | Evaluation harness with release gates | P3-01 | Gate failure blocks release | ⬜ |
| P3-12 | Per-call cost metering in Rand | P1-13 | Report per tenant | ⬜ |
| G3-A | **Gate A** review: accuracy, safety, cost | P3-11 | Recorded in WORKLOG | ⬜ |

## Phase 4: Ledger, autonomy, host, first UI (Gate B)
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P4-01 | `time-ledger`: append-only schema, supersede/void, units via `timelex-units` | P1-14, P0-04 | UPDATE/DELETE impossible for service role (test) | ⬜ |
| P4-02 | Policy engine and autonomy ladder; kill switch | P4-01, G3-A | Demotion on failed sample (test) | ⬜ |
| P4-03 | Review queue API; sampling audits | P4-02 | Sample rate honoured | ⬜ |
| P4-04 | `mcp-host` workflows: `attribute_block`, `narrate_entry` | P3-09 | Tools allow-listed; budget enforced | ⬜ |
| P4-05 | BYO-key onboarding (API + UI), spend caps, health checks | P1-05 | Revoked key degrades to `needs_review` | ⬜ |
| P4-06 | Web app v0: exceptions inbox, evidence view | P4-03 | Task time ≤ 5 min/day in usability test | ⬜ |
| P4-07 | Outlook add-in v0 | P4-03 | Assign/approve from Outlook | ⬜ |
| P4-08 | Claude plugin v0 (tools via gateway) | P1-08 | Plugin lists and calls tools | ⬜ |
| P4-09 | Memory store from confirmed corrections only | P4-04 | Injection cannot write memory (test) | ⬜ |
| P4-10 | Decide Managed Agents/Agent SDK vs own loop | T0-A01 | ADR update | ⬜ |
| P4-11 | Pilot with one firm, shadow mode, ≥ 4 weeks | P4-06 | Metrics report | ⬜ |
| G4-B | **Gate B** review incl. independent vault review | P4-11 | Recorded in WORKLOG | ⬜ |

## Phase 5: Billing
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P5-01 | Rate cards, discounts, arrangements (hourly, fixed, capped) | P3-04 | Cent-exact tests | ⬜ |
| P5-02 | WIP and ageing | P4-01 | Reconciles to ledger | ⬜ |
| P5-03 | Pro-forma assembly and certification workflow | P5-02 | Certify/reject/correct | ⬜ |
| P5-04 | ClearCut client; `ProFormaRequest` contract tests | P5-03 | Contract test against ClearCut OpenAPI | ⬜ |
| P5-05 | Exports: CSV/JSON pro-forma; GhostPractice-ready import file (if allowed) | T5-L01 | Totals reconcile | ⬜ |
| P5-06 | Fee-note delivery policy (opt-in per client) | P5-04 | Never auto-sends by default | ⬜ |
| P5-07 | Outside-counsel-guideline rules (optional) | P5-03 | Rule engine tests | ⬜ |
| P5-08 | Reports: realisation, utilisation, WIP | P5-02 | Match hand calc | ⬜ |
| P5-09 | Self-host packaging (Helm/Compose) | P1-07 | Fresh install < 30 min | ⬜ |

## Phase 6: Practice core
| ID | Task | Status |
|---|---|---|
| P6-01 | Full matters and conflicts check | ⬜ |
| P6-02 | Contacts / CRM | ⬜ |
| P6-03 | Tasks, key dates, court calendar | ⬜ |
| P6-04 | Document store, versioning, search | ⬜ |
| P6-05 | Document assembly (templates; qualified-person review gate; AI disclosure) | ⬜ |
| P6-06 | FICA onboarding workflows | ⬜ |
| P6-07 | E-signature integration | ⬜ |
| P6-08 | Mobile app or PWA | ⬜ |
| P6-09 | Conveyancing integration research (Conveyancer/Unity) | ⬜ |

## Phase 7: Trust and regulated accounting
| ID | Task | After | Status |
|---|---|---|---|
| G7-01 | **Gate C:** attorney + auditor read Part XII with us; sign scope; agree independent audit. Include the 54.1 scope check and the 54.9.2.1 hosted-records conditions ([research 03](research/03-south-africa-compliance.md)) | T0-L03, T0-L06 | ⬜ |
| P7-01 | Business and trust ledgers with invariants (no debit creditor, no commingling) | G7-01 | ⬜ |
| P7-02 | Receipts and payments with maker-checker | P7-01 | ⬜ |
| P7-03 | Monthly reconciliation (rule 54.15) | P7-01 | ⬜ |
| P7-04 | LPC reports and audit pack | P7-03 | ⬜ |
| P7-05 | Backups, retention, readability (54.14.3–54.14.4) | P7-01 | ⬜ |
| P7-06 | Interest and Fund transfers | P7-02 | ⬜ |
| P7-07 | Independent audit of the module | P7-06 | ⬜ |

## Phase 8: Migration, hardening, GA
| ID | Task | Status |
|---|---|---|
| P8-01 | GhostPractice import tooling with dry-run and reconciliation | ⬜ |
| P8-02 | Adapters: Clio, LegalSuite, others | ⬜ |
| P8-03 | `insights`: utilisation, forecasting, anomaly detection. Per-fee-earner performance views are an Anthropic high-risk use (qualified-person review, AI disclosure, never autonomous; [research 07](research/07-anthropic-terms-and-key-custody.md) §6) | ⬜ |
| P8-04 | POPIA review; retention and deletion tooling | ⬜ |
| P8-05 | Threat-model refresh; penetration test; independent vault review | ⬜ |
| P8-06 | SBOM, signed releases, provenance | ⬜ |
| P8-07 | Hosted offering in a South African region; SLA; status page | ⬜ |
| P8-08 | v1.0 criteria and release | ⬜ |
