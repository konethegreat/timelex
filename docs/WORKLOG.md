# Worklog

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Append an entry at the end of every working session (newest first).

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
