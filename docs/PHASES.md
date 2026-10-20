# Phases

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

Legend: ✅ done · 🔄 in progress · ⬜ not started. Estimates are rough, for **one part-time developer with AI assistance**, and will move after the first gold-set run. Task-level detail: [TASKS.md](TASKS.md).

**Order of value:** prove capture and attribution → autonomy with evidence → billing → practice core → regulated money → migration at scale. Each phase ends at a **gate**; we do not start the next phase's expensive work until the gate is passed or consciously waived (and the waiver recorded in [WORKLOG.md](WORKLOG.md)).

```
P0 Foundations ─► P1 Platform ─► P2 Capture ─► P3 Understand ─► P4 Ledger+Autonomy ─► P5 Billing
                                                    │ GATE A (accuracy)   │ GATE B (pilot)
                                              P6 Practice core ─► P7 Trust (GATE C) ─► P8 Migrate+Harden+GA
```

## Phase 0: Foundations and research ✅ / 🔄
**Goal:** know what we are building and why; publish a credible repository.
**Scope:** research 01–06, ADR-0001…0010, vision, architecture, autonomy, threat model, contracts v0, a tested billing-unit library, gateway health endpoint, Python redaction worker, TypeScript SDK stub, CI, GitHub hygiene.
**Exit criteria:** repo public and licensed; CI green; docs indexed; validation tasks (T0-*) listed with owners.
**Open validations (run in parallel with Phase 1):** T0-R01 GhostPractice demo/interviews, T0-B01 fee-earner interviews, T0-L01…L04 legal review and provider terms.
**Estimate:** done in this commit, except validations.

## Phase 1: Platform kit (≈ 4–6 weeks)
**Goal:** a walking skeleton of the platform every later service reuses.
**Scope:** per-service Postgres schemas and roles, tenancy with row-level security, identity, gateway (authN, metering), vault v0, outbox, OpenTelemetry, Docker Compose stack, MCP stateless adapter, SDK generation, service generator, branch protection.
**Exit criteria:** `docker compose up` runs gateway + identity + vault + Postgres + MinIO; a cross-tenant read test fails as designed; one end-to-end authenticated call returns through gateway to a service over REST **and** MCP; contract tests run in CI.
**Deployables:** ≤ 4.
**Risk:** over-building the kit. Rule: build only what the next service needs.

## Phase 2: Capture (≈ 6–8 weeks)
**Goal:** reliably turn real work signals into normalised activity events, with privacy controls.
**Scope:** `capture-hub`, Microsoft 365 mail/calendar/meetings, subscription renewal and lifecycle handling, backfill, quick-capture, privacy filters and redaction, consent/disclosure flows, gap detector; Windows desktop-agent spike then v0; Google research.
**Exit criteria:** one real mailbox (consented test account) captured for two weeks with no missed renewals; events idempotent and replayable; capture can be paused and excluded by domain/client; redaction verified by tests.
**Deployables:** ≤ 4 (gateway, identity+vault, capture-hub, mcp-host).
**Risk:** Graph permissions and tenant policies vary; transcripts may be unavailable. Fallback: calendar-only meeting time.

## Phase 3: Understand: segment, attribute, narrate (≈ 6–8 weeks) ⭐ GATE A
**Goal:** prove accuracy before building autonomy.
**Scope:** gold sets (synthetic public; consented private), segmentation, minimal `matters`, attribution cascade, calibration, narrative service with number guard, cost metering, prompt-injection red-team corpus, CI evaluation gates.
**Exit criteria (Gate A, proposed):** on the gold set, **0 silent critical errors**, attribution precision ≥ 98% at the chosen auto-post threshold with recall high enough to be useful (target ≥ 90% of billable time proposed), 100% narratives pass the number guard, 100% jobs reach a terminal state, red-team corpus passes, cost per entry reported in Rand.
**If Gate A fails:** stay at L2 (draft and approve) and iterate; do not build L3/L4 policy automation on unproven accuracy.

## Phase 4: Ledger, autonomy, host and first UI (≈ 6–8 weeks) ⭐ GATE B
**Goal:** a firm can use it daily in shadow mode.
**Scope:** `time-ledger` (append-only), policy engine and autonomy ladder, review queue and sampling audits, `mcp-host` workflows, BYO-key onboarding and spend caps, web app v0 (exceptions inbox, evidence view), Outlook add-in v0, Claude plugin v0, memory store from confirmed corrections, decision on Managed Agents vs own loop.
**Exit criteria (Gate B):** one pilot firm runs ≥ 4 weeks in shadow mode; measured auto-post rate, precision, review minutes per day and recovered time are reported honestly; kill switch tested; independent review of the vault design.
**Risk:** UX of exceptions determines adoption more than model quality.

## Phase 5: Billing (≈ 4–6 weeks)
**Goal:** from posted time to a certified pro-forma and exportable fee-note data.
**Scope:** rate cards and billing arrangements, WIP and ageing, pro-forma assembly, certification, ClearCut integration (maths, VAT, exports), CSV/JSON and GhostPractice-ready import files (subject to T5-L01), reports (realisation, utilisation), self-host packaging.
**Exit criteria:** a month of pilot time becomes pro-formas that the firm's billing staff accept with ≤ agreed corrections; totals reconcile to the cent against the firm's own calculation.
**Dependency:** ClearCut persistence and export progress ([ADR-0008](adr/0008-clearcut-relationship.md)).

## Phase 6: Practice core (≈ 12+ weeks, can overlap)
**Goal:** be useful to a firm without GhostPractice's front office.
**Scope:** full matters/contacts/conflicts, tasks and key dates, court calendar, document store and search, human-reviewed document assembly, FICA onboarding workflows, e-sign, mobile, conveyancing integration research.
**Exit criteria:** pilot firm can run intake → matter → tasks → documents in TimeLex for new matters.

## Phase 7: Trust and regulated accounting (≈ 12+ weeks, **GATE C**)
**Gate C (must pass before any code):** an attorney and a registered auditor have read the full LPC Part XII text with us, signed off the scope, and agreed an independent audit of the finished module (G7-01).
**Scope:** business and trust ledgers with invariants, receipts and payments with maker-checker, monthly reconciliation (rule 54.15), LPC reports and audit pack, backups and readability (54.14.3–54.14.4), interest transfers.
**Exit criteria:** independent review passes; a pilot firm's auditor accepts the output. **Never autonomous.**
**Alternative:** integrate a partner's regulated ledger rather than build ([research 06](research/06-strategy-and-positioning.md) §2).

## Phase 8: Migration, hardening and GA (ongoing)
**Scope:** GhostPractice import tooling, other adapters (Clio, LegalSuite), insights/forecasting, POPIA review and deletion tooling, penetration test, SBOM and signed releases, hosted offering in a South African region, v1.0 criteria.
**Exit criteria (v1.0, proposed):** ≥ 3 firms in production, security review passed, published measured accuracy and cost numbers, documented incident process.

## Cross-cutting tracks (every phase)
| Track | Standing tasks |
|---|---|
| Security | Dependency scanning, secret scanning, threat model review, key hygiene |
| Compliance | Keep research 03 current; lawyer reviews; POPIA records |
| Evaluation | Gold sets, CI gates, weekly sampling |
| Docs | ADR for every significant decision; worklog entry per session |
| Cost | Rand-denominated cost reporting for model and cloud spend |
| Community | Issues, templates, good-first-issues, release notes |

## Decisions already made
See [ADRs](adr/) 0001–0010 and the "pushback" items in [research 06](research/06-strategy-and-positioning.md).
