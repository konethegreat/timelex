# Worklog

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Append an entry at the end of every working session (newest first).

## 2026-10-31 (latest): gateway routes the vault — contract 0.4.0
**Trigger:** P1-05 increment 3 merged as `09f33bb` (#21), and the dependabot queue was cleared; the vault had no caller yet.

**Done**
- [`contracts/openapi/gateway.v1.yaml`](../contracts/openapi/gateway.v1.yaml) 0.4.0: `GET /v1/secrets` (optional `includeRevoked`), `PUT /v1/secrets/{kind}`, `DELETE /v1/secrets/{id}`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, each requiring a tenant API key and referring to the vault contract's schemas. 409 and 422 pass through; 401 and 5xx are still hidden by the gateway.
- [`services/gateway/src/proxy.rs`](../services/gateway/src/proxy.rs): a `Target` describes each REST route (label, service, method, canonical path, the 400 detail when it has no path). `internal_token` now takes the audience, and `forward` takes the service and method, so the same `handle` serves identity and the vault. Tokens for secrets are minted for `vault`.
- Validation before forwarding: the four kinds against a fixed list, ids by parsing and re-formatting as UUIDs, and the list query accepted only as absent, `includeRevoked=true` or `includeRevoked=false`. A bad parameter is a 400 *after* authentication and rate limiting, and nothing reaches the vault.
- [`services/gateway/tests/routing.rs`](../services/gateway/tests/routing.rs): 3 stub tests (forwarding, audience and body; canonical paths, query and 400s; an unconfigured vault is 502). [`services/gateway/tests/e2e.rs`](../services/gateway/tests/e2e.rs): a real vault service with its own schema, role and migrations in the same throwaway database, 3 tests (store/list/test/rotate/revoke with a `settings:manage` refusal, cross-firm invisibility, and the vault refusing tenant keys, forged tokens and operator tokens). 27 routing and 9 end-to-end tests pass.

**Decisions and why**
- **PUT and DELETE share `/v1/secrets/{key}`**, as the vault's router does: axum will not register one segment under two parameter names, and the wire paths stay exactly the vault contract's.
- **The gateway validates kinds and ids itself.** It has no business rules otherwise, but these two decide a URL, and forwarding a client's raw text would let `..%2f` tricks and unknown kinds reach the vault. Canonical re-formatting keeps every forwarded URL one the contract describes.
- **One query parameter, positively allowed.** `includeRevoked` is the only query the route has; anything else is refused rather than dropped, so a caller cannot believe a filter was applied when it was not.
- **A vault with no configured URL is 502**, not 501: the route exists, the service behind it is unreachable, and the contract already says 502 for that.
- **The permission stays the vault's.** The gateway forwards the principal's permissions and the vault enforces `settings:manage` in the store, so a new route cannot forget the check.
- **Test values are built at run time** (`sk-ant-api03-<uuid>`), and the end-to-end test builds the vault master key from two UUIDs, so no credential-shaped literal enters the repository.

**Next:** P1-06 outbox and relay, starting with `secret.*` events; then P1-15 MCP tools for the vault and P1-13 `mcp-host`.

## 2026-10-30 (latest): P1-05 vault v0 finished — HTTP API and the Anthropic check
**Trigger:** increment 2 merged as `8ef8214` (#20); the last part is the service the gateway and the admin UI will talk to.

**Done (increment 3)**
- [`services/vault/src/http.rs`](../services/vault/src/http.rs): all five contract routes plus `/health`. Internal tokens of kind `principal` only; the firm, user and permissions come from the verified token. Problem details (RFC 9457) for every failure, a 64 KiB body limit, and metadata JSON that omits `lastTestedAt`, `lastUsedAt` and `revokedAt` until they exist instead of sending null.
- [`services/vault/src/validator.rs`](../services/vault/src/validator.rs): a `ProviderValidator` trait, so the HTTP layer is tested with a script and no network. The live validator makes one read-only `GET https://api.anthropic.com/v1/models?limit=1` for `anthropic_api_key` with the key in `x-api-key`, and reads only the status: 2xx accepted, 401/403 rejected, anything else (429, 5xx, timeout, refused connection) unreachable. Other kinds pass on shape, as 0.1.0's contract says.
- [`services/vault/src/main.rs`](../services/vault/src/main.rs): `serve` and `migrate`; the master key is base64 of exactly 32 bytes, decoded into a zeroising wrapper, and no error message prints it.
- 5 unit tests (status mapping, header placement against a local stand-in server, timeout/refusal) and 13 HTTP tests against Postgres: create/list/revoke, replace under the same id, rotation, five credential failures mapping to one 401, wrong kind and missing permission mapping to 403, malformed and 70 KiB bodies (400/413), unknown kind and a host outside the kind (400), a passing test stamping only `lastTestedAt`, a rejection as 422, an outage as 502 with no stamp, and another firm's id as 404 for every operation.
- Every response in those tests is searched for the full value; it never appears. `Debug` for `SecretValue`, `LoadedSecret` and `PutSecretRequest` is redacted, which the tests assert, and the only log line the service writes is a fixed sentence plus a database or rule message that never holds a value.

**Decisions and why**
- **Axum cannot register `{kind}` and `{id}` on the same segment**, so one route owns `/v1/secrets/{key}`: PUT reads it as a kind, DELETE as a UUID. The wire paths are the ones the contract describes.
- **An unreachable provider is not recorded as a test.** A 502 means no verdict; writing `lastTestedAt` would suggest the key failed and could push an admin to revoke a working key. A rejection is a verdict and is recorded.
- **The provider's own words are never relayed**, and a rejection body is fixed text. A provider error can contain the key or account details.
- **`includeRevoked` is parsed by our own extractor**, so a bad query is a problem-details 400 instead of axum's plain-text rejection.
- **Test keys are drawn at run time** (uuid 4), after CodeQL flagged fixed key and nonce literals in increment 2 as hard-coded cryptographic values.

**Still to come:** a gateway route for `/v1/secrets` (the next password-like settings also need `settings:manage`); `secret.*` events through the outbox (P1-06); the lease (P1-13); MCP tools through the P1-15 adapter; KMS/WIF (T0-L05).

## 2026-10-30 (later): P1-05 vault v0 — rules, sealed schema and store
**Trigger:** increment 1 (contract and docs) merged as `9a7c066`; continuing with the part that has to be right before any endpoint exists.

**Done (increment 2)**
- [`crates/timelex-vault`](../crates/timelex-vault), a no-I/O crate: the four kinds and their provider hosts; validation of value (printable ASCII, 8–4096), label and hosts (narrowing only, never widening); a zeroising `SecretValue` with redacted `Debug`; envelope encryption (AES-256-GCM per secret, data key wrapped behind a `KeyWrapper` trait) with tenant, secret id and kind bound into **both** seals; and the per-firm audit hash chain. Entropy and the clock are parameters, so every rule is tested with fixed inputs (27 unit tests, 1 doctest).
- [`services/vault/migrations/0001_vault.sql`](../services/vault/migrations/0001_vault.sql): `secrets` (sealed columns NULL together, coarse CHECKs as a second line, a partial unique index giving one live secret per kind, a CHECK that a revoked row cannot hold a value), `secret_audit` (append-only; no UPDATE or DELETE grant) and `chain_heads`. All three get the standard tenant policy; `vault` is provisioned in the bootstrap.
- Store (`services/vault/src/store.rs`): put (create or replace), list metadata, rotate, revoke, open-for-test, record-test. Each runs in one tenant transaction and appends one audit entry under a `FOR UPDATE` lock on the chain head, so chains cannot fork. Revocation NULLs every sealed column.
- 11 database tests: value never stored, replace destroys the old value, rotate clears the test stamp, revoke keeps metadata, another firm sees nothing, no permission writes nothing, the database refuses a clear value / bad host / rewritten audit / deleted secret, every vault table has the policy, two firms have chains that both start at 1.

**Decisions and why**
- **Binding in the seals, not just the row.** Copying a row to another firm, or relabelling a key's kind, makes it fail to open rather than be used against the wrong provider.
- **Replacement, rotation and revocation all destroy the old data key**, so a database copy taken later cannot decrypt a superseded value. Revoked rows keep metadata (provider, last four, when) and nothing else.
- **One live secret per kind is a database guarantee**, not a code convention: a partial unique index. A PUT racing another PUT retries once and replaces.
- **The audit chain is per firm and written in the same transaction as the change.** A separate audit writer would allow a change with no entry.
- **Testing is not use:** `lastTestedAt` is set by `record_test`; `lastUsedAt` is reserved for leases and stays NULL in v0.

**Next:** increment 3 — the HTTP layer (internal `principal` tokens, `settings:manage`, problem details, 64 KiB limit), the validator trait with one real Anthropic check, the binary, and HTTP tests proving no plaintext is returned or logged.

## 2026-10-30 (even later): P1-05 vault v0 starts with the contract
**Trigger:** After gateway part b merged (#18), Kone chose P1-05 (vault v0) next, with REST only in this version (MCP waits for P1-15) and a validator trait plus one real Anthropic check.

**Done (increment 1)**
- [`contracts/openapi/vault.v1.yaml`](../contracts/openapi/vault.v1.yaml) 0.1.0: `PUT /v1/secrets/{kind}` (create or replace), `GET /v1/secrets` (metadata only), `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}` (revoke, idempotent). Typed kinds (`anthropic_api_key`, `m365_oauth`, `google_oauth`, `generic_api_key`), allowed hosts that may narrow but never widen a kind's defaults, and metadata that never contains a value.
- Callers are internal tokens of kind `principal` holding `settings:manage`; the firm comes from the token (ADR-0012). Only the gateway reaches the service.
- Vault README, architecture table and task list updated. OpenAPI validated with `openapi-spec-validator` (all three contracts pass).

**Decisions and why**
- **REST only in v0.** The lease (`/internal/v1/secrets/{id}/lease`) has no consumer until `mcp-host` (P1-13) and is the most dangerous surface, so it lands with its first caller and its own tests. The MCP tools are served through the P1-15 adapter, not hand-rolled twice.
- **`settings:manage` rather than a new permission.** Secrets are firm settings and the permission is human-only; adding `secrets:*` would mean touching identity's permission model and contract for no v0 gain. A dedicated permission can come with the lease, when non-human callers need scoped access.
- **Write-only by shape.** There is no GET by id and no endpoint that returns a value; a caller gets the last four characters so two keys can be told apart.
- **Revoke destroys the wrapped data key**, not just a flag, so a later database copy cannot decrypt the value.
- **Test is not use.** `lastTestedAt` and `lastUsedAt` are separate; only an internal caller using the secret sets the latter.
- **KEK from the environment behind a `KeyWrapper` trait** for 0.1.0 (KMS and WIF wait on T0-L05), so the storage format does not change when a KMS adapter arrives.

**Next:** increment 2 — `crates/timelex-vault` (kinds, validation, zeroising value, envelope encryption) plus the `vault` schema, bootstrap role and store with database tests.

## 2026-10-30 (later still): P1-04 part b — MCP header routing
**Trigger:** PR B merged (#17); finishing part b with `POST /mcp`.

**Done (PR C)**
- `services/gateway/src/mcp.rs`: compiles `contracts/mcp/routes.json` into the build (`include_str!`), refusing duplicates, empty names and an empty table at start-up. `classify` decides from the three headers alone: version must be `2026-07-28`, `Mcp-Method` is validated, and only `tools/call` needs `Mcp-Name`. A valid method it does not route is `Unrouted`, not malformed.
- `services/gateway/src/proxy.rs`: `POST /mcp` authenticates the tenant key, spends the firm's budget, classifies the headers, looks the tool up, mints a token addressed to the *owning* service and forwards the body untouched with an allowlist of headers (`Content-Type`, the three MCP headers, `Origin`). Unknown tool 404; unrouted method or unconfigured service 501; malformed headers 400 after authentication.
- `Trip` now carries route, service and tool; the meter record gains optional `service` and `tool`.
- 8 new routing tests with a second stub service: routing and audience, the header allowlist, an identity-owned tool, unknown tool, 501s, malformed headers (and 401 before any 400), operator keys refused, and the firm budget covering MCP.
- README, TASKS and the worklog updated. Contract unchanged (already 0.3.0 from PR A).

**Decisions and why**
- Routing happens after authentication and the budget check, like the REST routes, so an unauthenticated caller cannot probe the table and a firm at its limit cannot make the gateway read a body.
- `Origin` travels on because the service must enforce it (MCP 2026-07-28); nothing else the client sent does.
- The token's audience is the owning service, not identity, so a tool call cannot be replayed against identity.
- The table is compiled in, so a deployment carries no file that can drift from the code.

**Verified:** `cargo fmt --all`, `cargo clippy -p timelex-gateway --all-targets -- -D warnings`, `TIMELEX_SKIP_DB_TESTS=1 cargo test -p timelex-gateway` (25 unit + 2 contract + 24 routing + 6 end-to-end passing; the end-to-end tests pass early locally and run against Postgres in CI).

**Next:** P1-05 (vault v0), P1-06 (outbox) or P1-15 (`timelex-mcp` adapter and MCP aggregation).

## 2026-10-30 (later): P1-04 part b — rate limits and metering
**Trigger:** Continuing part b after the contracts PR merged (#16).

**Done (PR B)**
- `services/gateway/src/limits.rs`: token buckets keyed per firm and per operator; fake-clock tests for burst, refill, isolation and a clock going backwards. The map recovers from a poisoned mutex rather than taking the gateway down.
- `services/gateway/src/meter.rs`: one metadata-only JSON record per handled request to stdout; running totals per firm; `GET /internal/usage` payload with `since` and uptime.
- `services/gateway/src/config.rs`: `TIMELEX_SERVICE_URLS` (validated, `identity` required) replaces `TIMELEX_IDENTITY_URL`; `TIMELEX_RATE_LIMIT_PER_MINUTE`/`_BURST` (default 600/120); `TIMELEX_GATEWAY_INTERNAL_ADDR` (default `127.0.0.1:8089`). Bad values fail at start-up; zero is refused so a typo cannot lock a firm out.
- `services/gateway/src/proxy.rs`: authentication, then the budget check, then the body read and forward. One `Trip` records every exit path: 401, 429, 400, 413, 502/504 and success. 429 carries `Retry-After`.
- `app_with` now returns `Gateway { public, internal }` so the binary binds the usage listener separately and warns if it is not loopback. Tests updated; 3 new routing tests and 1 new end-to-end test cover 429, operator isolation, internal-only usage and per-firm totals.
- READMEs, architecture table and task list updated.

**Decisions and why**
- The budget is spent **after** authentication, so a 429 still costs one `authenticate` call. That is the price of not trusting the key prefix; documented as the known amplification gap rather than solved twice.
- The 429 request's body is never read and nothing is forwarded, so a firm at its limit cannot make identity or the service do work.
- Usage is served from a second listener, not the public one; a routing test asserts `/internal/usage` is a 404 on the public app and `/v1/api-keys` is a 404 on the internal one.

**Verified:** `cargo fmt --all`, `cargo clippy -p timelex-gateway --all-targets -- -D warnings`, `TIMELEX_SKIP_DB_TESTS=1 cargo test -p timelex-gateway` (20 unit + 2 contract + 16 routing passing; 6 end-to-end pass early without a database, CI runs them on Postgres).

**Next:** PR C (`POST /mcp` routed by `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` through `contracts/mcp/routes.json`, 501 for unrouted methods).

## 2026-10-30: P1-04 part b starts with contracts
**Trigger:** Kone asked to pick the system up again and chose gateway part b (rate limits, metering, `Mcp-Method` routing). Context reload confirmed the ClearCut boundary: billing calls ClearCut directly (ADR-0008); ClearCut is not a gateway target.

**Done (PR A, contracts only)**
- `contracts/openapi/gateway.v1.yaml` 0.3.0: 429 with `Retry-After` on every authenticated operation, a `POST /mcp` operation routed by `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name`, and 501 for methods not routed yet.
- `contracts/mcp/routes.json`: tool-to-service routing for all 26 tools in the catalogue, with read-only flags.
- `services/gateway/tests/contracts.rs`: parses `tools.md` and fails if the catalogue and `routes.json` drift (2 tests pass).
- [ADR-0013](adr/0013-gateway-limits-and-metering.md) accepted: per-firm token buckets after authentication, metadata-only metering to stdout plus a loopback `/internal/usage`, header routing without body parsing, and the config changes.
- Docs index, contracts README and MCP catalogue updated.

**Decisions and why**
- Limits are keyed **after** authentication. The tenant id in the key prefix is unverified; a pre-auth bucket could be spoofed to burn another firm's budget.
- The limiter is in process, so the effective limit multiplies with replicas until a shared store exists. Phase 1 runs one gateway.
- Metering stays out of Postgres: the gateway owns no data, and P1-06/P1-10 become the durable path. This is request metering, not model spend (P3-12).
- `tasks/*` cannot be routed by `Mcp-Name` because it carries the task id; P1-15 must prefix task ids with a service or add a lookup.

**Known gaps**
- Invalid keys still reach identity on every attempt; identity-side or edge per-IP limits are the later fix.
- SSE/streaming, OAuth, tool schemas and MCP aggregation wait for P1-15.
- Usage counters reset on restart; `since` and uptime are in the response.

**Next:** PR B (rate limits and metering), then PR C (MCP routing).

## 2026-10-21: Merged the open PRs
**Trigger:** Kone asked to merge every open PR and close any that should not be merged.
- **Merged, in order, with merge commits** (so each stacked PR still applied cleanly after its base landed): #4 identity domain crate, #5 database layer and schema, #6 identity store and contract, #7 P1-04 contracts and ADR-0012, #8 P1-04 part a (internal token, identity HTTP, gateway authentication and routing); then Dependabot #3 (GitHub Actions bumps, pinned by SHA).
- **Closed, not merged:** #1 `postgres:17` to `postgres:18`. Tested: the container exits because the Compose volume is mounted at `/var/lib/postgresql/data`, a path the 18 image no longer supports. Moving to 18 means changing the mount and re-running the database tests and CI on 18; it needs its own PR.
- **#2** (`jsonschema` 0.30 to 0.59) conflicted with `Cargo.lock` after the merges; Dependabot was asked to rebase it. Outcome below if resolved this session.
- GitGuardian flagged synthetic test credentials on #8 (Basic auth header, key-shaped Bearer literal). Fixed by building them at run time; the rule is in CLAUDE.md now.
- CLAUDE.md status sections refreshed (they still described the skeleton).

## 2026-10-20 (session 3): Step 3 begins: P1-01 to P1-03 (schema, tenancy, identity domain)
**Trigger:** Kone approved the Step 1 report and the drafted schema (tenant-in-key tokens, no passwords, identity-only provisioning).

**Done**
- Pushed the Step 1 commits to `main`. From here, work goes through small PRs.
- PR 1: `timelex-identity` domain crate (pure; 55 unit tests + doctest). PR 2 (stacked on PR 1): bootstrap SQL, identity migration, `timelex-db` crate (`TenantPool`/`TenantTx`), 18 database tests, CI Postgres service, ADR-0011.
- Applied the SQL for the first time to a throwaway Postgres 17 (trust auth, no passwords); all 18 tests passed first run, then a **mutation check** (policy replaced by `USING (true)`) made 7 of 11 tenancy tests fail, so the tests do bite.

**Later in the session (PR 3, stacked on PR 5):** `contracts/openapi/identity.v1.yaml` (validated with `openapi-spec-validator`), two domain helpers (`require_permission`, `effective_permissions`), the DB test harness moved into `timelex_db::testing` (feature `testing`), and the identity store crate `timelex-identity-service`: create firm, issue key, revoke key, authenticate, with 16 database tests. Mutation checks: skipping the usable-key check broke 4 tests; dropping role narrowing broke 1.
- **Design finding:** a key's permissions are its scopes intersected with the owner's *current* roles, so demoting someone shrinks their existing keys at once.
- **Open (decide later):** `keys:manage` lets a holder issue a key up to any user's permissions (documented; only admins have it). Idempotency keys (ADR-0010) are accepted but ignored: replaying key issuance cannot return the token twice, so it needs a design decision.

**PR 4 (stacked on PR 3): P1-04 part a.** Contracts first (ADR-0012 accepted; `internal-auth.md`; identity and gateway OpenAPI 0.2.0), then: `timelex-auth` (EdDSA internal token, 20 tests including alg-none, HMAC-with-public-key, forged-key, wrong-audience and tampered-claim attacks); identity HTTP layer and binary (11 tests); gateway authentication and routing (13 stub-upstream tests, 5 end-to-end tests with Postgres); a manual smoke test of the real binaries with `curl`.
- Mutation checks (by hand): operator check bypassed, identity ignoring caller kind, and a leaked extra header each broke at least one test. The leak mutation was *not* caught at first (the test blocked named headers); it now uses an exact header allowlist.
- **Finding:** reqwest cannot tell a connect timeout from a response timeout, and on Windows a closed port times out instead of refusing, so "service down" can read as 504 there. The contract wording still holds ("outcome unknown"); the test uses a dropped connection instead of a closed port.
- **Duplicated on purpose for now:** the problem-details helper exists in both the gateway and identity. A shared HTTP kit crate is worth extracting when a third service needs it.
- **Open:** the gateway has no tracing or request ids (P1-10); identity logs refusal reasons to stderr only; operator credential is a single static key hash (no operator accounts); replay protection is the 30-second lifetime only.

**Not done:** gateway part b (rate limits, metering, `Mcp-Method` routing), users/roles/SSO, audit events.
**Unverified:** behaviour on Postgres 18 (Dependabot PR open); hosted Postgres with a restricted admin role (the bootstrap needs `CREATEROLE`); performance of the policy on large tables.

## 2026-10-20 (session 2): Step 1 research pass, no product code
**Trigger:** research in `docs/research/` was partly unverified; Kone asked for the gaps to be closed from primary sources before building (Step 1), with a stop for review before P1-01 to P1-03.

**Done**
- **T0-L03 / T0-L04 (research 03):** read Part XII (rules 54.1, 54.6–54.15, 54.19, 54.20, 54.24, 54.34, 57) in the Acts Online republication of Notice 401 of 2018, the LPC advisory of 10 May 2024 with the court order, POPIA ss71, 72, 21, 22, and the Information Regulator's guidance-notes page. Listed ten software duties. Corrected: Part XII scope (54.1), retention clock (from last entry), audit-report citation (54.24 not 57.2), client-account content (agreed fee flag), payment methods (54.14.15); **s71 is narrower than we wrote**; added 54.9.2.1 (hosted records). Regulator's transfer guidance: not published, announced as planned.
- **T0-L02 (new research 07):** public Anthropic terms, DPA, retention, ZDR, residency, usage policy, key types, workspaces, Workload Identity Federation. Drafted nine questions for Anthropic.
- **P1-08 (new research 08, spike in `docs/research/spikes/p1-08-rmcp/`):** `rmcp` 3.6.0 serves 2026-07-28 statelessly with Tasks; 18 raw probes; defaults need configuration (Origin, session mode, required metadata).
- **GhostPractice (research 01 §8):** 26-row claim register from the vendor's own pages and the acquirer's press release.
- **Kits:** `docs/validation/interview-kit.md` and `demo-checklist.md` for T0-R01 and T0-B01.
- ADR amendments **proposed** in 0003, 0004, 0005, 0007, 0009 (Decision text untouched). Threat model T19–T24 added. TASKS updated (P1-08 done server-side; new P1-15, T0-L05, T0-L06, T0-A03).

**Findings that change plans**
- **Anthropic has no African inference region** (`global` or `us` only); Fable-class models need 30-day retention and **cannot use ZDR**, which breaks the top rung of the escalation ladder for ZDR tenants.
- **Workload Identity Federation** could let TimeLex hold no Anthropic secret; proposed as a second mode, waiting for Anthropic's written answer.
- The usage policy treats **evaluating a worker's performance** as high-risk, which constrains per-fee-earner analytics.
- GhostPractice is **not "Windows on-premise only"**: hosting is the firm's choice, the database is Microsoft SQL, remote-desktop and virtual-workstation access is listed, and **import/export is advertised**. The vendor is silent on an API. The owner opened a strategic review in July 2025 (unverified for 2026).
- ClearCut's note that `rmcp` 3.5.1 spoke 2025-06-18 does not match the crate changelog or the spike (3.5.1 not tested).

**Still unverified / open**
- Part XII from the Gazette itself (we read a republication through a summarising tool; rules 54.2–54.5, 54.16–54.18, 54.21–54.23, 54.25–54.33, 55, 56 unread). POPIA text is from a commentary site, not the Gazette.
- Anthropic's position on key custody and a vendor-run federation issuer; DPA coverage of POPIA; clause numbers as reported by our fetch tool.
- Capterra facts (price, Windows) could not be re-checked (HTTP 403). GhostPractice export formats, licence terms, API.
- `rmcp` client side, `tasks/update`, `tasks/cancel`, auth, load; the `Mcp-Name` rule for task methods.
- Whether captured time or fee records count as "accounting records" under rule 54.9 (counsel).
- Everything in the interview kit's hypotheses H1–H8: no interview has happened.

**Plan impact:** none to phase order or gates. Phase 1 gets one extra task (P1-15) and Phase 1's MCP risk is lower; vault scope in P1-05 stays "stored key only" until Anthropic answers. Step 3 is not started.

**Next:** Kone reviews Step 2 report; then P1-01 to P1-03 (schema shown as SQL and explained before any migration).

## 2026-10-20: Restart with research; scaffold and publish
**Trigger:** Kone judged both earlier TimeLex prototypes too weak to replace GhostPractice and asked for a restart backed by research: multi-language, microservice, MCP-host backend, bring-your-own keys, connected to ClearCut, fully autonomous, public and credible.

**Done**
- Reviewed the two prototypes (static HTML v1; Next.js v2 with a simulated sync gateway) and ClearCut (Rust core, ADRs, research, phases). Adopted ClearCut's conventions: ADRs, research files, PHASES/TASKS, Apache-2.0, "Written by Kone Tshivhinda & Claude", Rand-first costs.
- Researched GhostPractice, the AI time-capture competitors, South African constraints (LPC Accounting Rules Part XII, POPIA s72, Anthropic usage policy), Microsoft Graph capture limits and MCP 2026-07-28. Files `docs/research/01–06`.
- Wrote ADR-0001…0010, vision, architecture, autonomy ladder, threat model, glossary, phases and a 100+ task backlog.
- Built a walking skeleton in three languages: `timelex-units` (14 tests + doctest), `timelex-types` (7 tests, schema-validated), `timelex-gateway` (3 tests), `py-worker` redaction (19 tests), TypeScript SDK stub (3 tests).
- Specified twelve services; contracts v0; plugin placeholder; CI, CodeQL, dependency review, Scorecard, Dependabot, labels, issue/PR templates, governance, security policy.

**Pushback recorded (see research 06)**
1. "Replace GhostPractice" is three products (time/billing, practice core, regulated money). Start with the first; trust accounting is last and gated.
2. "Fully autonomous" is implemented as exception-based, measured autonomy. Fee notes, trust money and legal documents stay human-gated.
3. Microservices need guardrails for one part-time developer: contract-first, service budget, platform kit; merge services behind the same contracts if the kit does not pay off by end of Phase 2.
4. Language choice is by fit (correctness, ecosystem), not raw speed; three-language ceiling.

**Findings that change plans**
- GhostPractice lists **no API** and is Windows on-premise (third-party listings): interop must be through sanctioned exports/imports (ADR-0009).
- Every AI time-capture product found still requires human review of drafts: the autonomy gap is real and also a risk.
- Outlook Graph subscriptions expire in under 7 days (under 1 day for rich notifications), so renewal and gap detection are first-class.
- Illustrative model spend (R13–R56 per fee earner per month) is far below competitor licences (R700–R3 000+), which supports BYO keys; unmeasured.
- Anthropic usage policy requires qualified-person review for legal advice and documents prepared for signing/filing.

**Unverified / open**
- All GhostPractice facts come from third-party listings and a marketing page (no demo seen).
- LPC rules were read as machine summaries; verify against the primary text (T0-L03).
- Information Regulator final transborder guidance not found (T0-L04).
- Rust MCP SDK maturity versus the final spec (P1-08).
- Anthropic retention/ZDR and key-custody terms (T0-L02).

**Next:** see "Immediate next five" in `docs/TASKS.md`.

## 2026-10-20 (later): publication status

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

- Public repo `konethegreat/timelex` is live (Apache-2.0). CI, CodeQL, Sync labels and OpenSSF Scorecard all passed on the first commit.
- Enabled: Discussions, private vulnerability reporting, delete-branch-on-merge. Already on: secret scanning and push protection.
- **Still manual (browser automation was unreliable):** repository topics; dependency graph, Dependabot alerts and security updates (the "Dependency review" workflow fails on Dependabot PRs until the dependency graph is on); a light ruleset; the `v0.1.0` release (tag pushes were refused from the build sandbox, so create the tag in the release form).
- Dependabot has opened three version-update PRs (actions group, `jsonschema` 0.30 → 0.59, Postgres 17 → 18). CI passes on the first two; review each, and do not merge the Postgres 18 bump without checking the volume-layout change.
