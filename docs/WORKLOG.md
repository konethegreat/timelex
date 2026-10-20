# Worklog

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Append an entry at the end of every working session (newest first).

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
