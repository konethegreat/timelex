# Tasks

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

Legend: ✅ done · 🔄 in progress · ⬜ not started. `P#-##` build tasks; `T#-*` research, legal and validation tasks; `G#-##` gates. Dependencies in the "After" column. Every task is expected to produce: code + tests (or a decision record), and an update to this file and [WORKLOG.md](WORKLOG.md).

## Immediate next five
1. **T0-R01** GhostPractice demo or trial, plus three user interviews. _Kit ready:_ [interview-kit](validation/interview-kit.md), [demo-checklist](validation/demo-checklist.md).
2. **T0-B01** Five fee-earner interviews: how time is recorded today and what they would let a machine post.
3. **P1-01 → P1-03** Postgres schemas and roles, tenancy with row-level security, identity.
4. ~~P1-08~~ Server side verified 20 Oct 2026 ([research 08](research/08-mcp-rust-sdk-spike.md)). Next: **P1-15** adapter crate with contract tests; **T0-L05** send Anthropic the written questions.
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
| P1-03 | `identity`: tenants, users, roles, API keys | P1-02 | Create tenant, issue key, authenticate | 🔄 domain crate (55 tests) and schema done; identity store (create, issue, authenticate) and OpenAPI contract next |
| P1-04 | `gateway`: authN, rate limits, metering, header-based routing | P1-03 | Authenticated call routes to a service | ⬜ |
| P1-05 | `vault` v0: envelope encryption, write-only API, key test, audit. Stored-key mode only; WIF waits on T0-L05 ([ADR-0004 proposal](adr/0004-byo-keys-and-secrets-vault.md)) | P1-02 | Secret never returned, never logged (test) | ⬜ |
| P1-06 | Outbox + relay (Postgres) | P1-01 | Event published exactly-once-effect with idempotent consumer | ⬜ |
| P1-07 | `docker compose` dev stack (Postgres, MinIO, services, py-worker) | P1-01 | `docker compose up` healthy | ⬜ |
| P1-08 | Verify Rust MCP SDK vs final 2026-07-28 spec; else thin Streamable-HTTP adapter | | `tools/list`/`tools/call` over stateless HTTP | ✅ server side (`rmcp` 3.6.0, [research 08](research/08-mcp-rust-sdk-spike.md)); client side tested in P1-13 |
| P1-09 | OpenAPI → TypeScript SDK generation; contract tests in CI | P0-05 | Drift fails CI | ⬜ |
| P1-10 | Telemetry kit (OpenTelemetry): traces, metrics, redacted logs | | Trace spans across gateway→service | ⬜ |
| P1-11 | `tools/new-service` generator | P1-04, P1-10 | New service builds with auth, tenancy, health, CI | ⬜ |
| P1-12 | Branch protection / rulesets; required checks | P0-10 | PRs need green CI | ⬜ |
| P1-13 | `mcp-host` skeleton: `ModelProvider` trait, Anthropic adapter, budgets; MCP client side tested here; **escalation ladder is tenant-conditional (Fable-class models need a 30-day-retention workspace, [research 07](research/07-anthropic-terms-and-key-custody.md) §3)** | P1-05 | Mock-provider test; Rand cap aborts a loop; a model the key cannot use degrades to the next rung or `needs_review` | ⬜ |
| P1-14 | Hash-chained audit-log library | P1-01 | Tamper test detects edit | ⬜ |
| P1-15 | `timelex-mcp` adapter: safe `rmcp` config (stateless, Origin checks, required metadata, 2026-07-28 only), exact version pin, the spike's 18 probes as Rust contract tests | P1-08 | Probes pass in CI; an `rmcp` bump that breaks one fails CI | ⬜ |

## Phase 2: Capture
| ID | Task | After | Acceptance | Status |
|---|---|---|---|---|
| P2-01 | Activity-event contract v1 (+ versioning rules) | P0-05 | Backward-compat test | ⬜ |
| P2-02 | `capture-hub` framework: connector trait, idempotent ingest, replay | P1-06, P2-01 | Duplicate delivery yields one event | ⬜ |
| P2-03 | Microsoft 365 mail connector: admin consent, subscriptions, renewal, lifecycle | P2-02, P1-05 | 14 days, zero missed renewals on a test mailbox | ⬜ |
| P2-04 | Calendar and Teams meetings (attendance; transcripts where permitted) | P2-03 | Meeting durations match Teams reports | ⬜ |
| P2-05 | Google Workspace connector | T2-R01 | Mail+calendar parity with M365 | ⬜ |
| P2-06 | Windows desktop agent v0 (local filter, pause, signed build) | T2-R03 | Foreground doc titles captured; pause honoured | ⬜ |
| P2-07 | Privacy: exclusions (client/domain/matter), redaction pipeline, metadata-only default | P0-08 | Redaction test suite; excluded domains never stored | ⬜ |
| P2-08 | Manual quick-capture API (+ mobile web) | P2-02 | Entry becomes an activity event | ⬜ |
| P2-09 | Gap detector (calendar/activity vs posted time) | P2-04 | Flags a seeded gap | ⬜ |
| P2-10 | Consent and disclosure UX for admins and users | P2-07 | Data-flow register export | ⬜ |
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
