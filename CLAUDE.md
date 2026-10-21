# CLAUDE.md: TimeLex

Written by Kone Tshivhinda & Claude. Claude Code loads this file automatically.

## What this is
TimeLex: an open, API-first, **exception-based autonomous** practice-management platform for South African law firms. Wedge: capture → attribute → narrate → post → pro-forma. Later: practice core, then (gated) trust accounting. Many small services, each usable over REST, MCP and a plugin. Customers bring their own API keys. Sibling project: [ClearCut](https://github.com/konethegreat/clearcut) (invoices). Public repo: github.com/konethegreat/timelex (Apache-2.0).

## Read these first (in order)
1. `docs/PHASES.md` and `docs/TASKS.md`: roadmap, gates, task IDs. Work in that order unless told otherwise.
2. `docs/research/06-strategy-and-positioning.md`: scope, pushback, risks.
3. `docs/AUTONOMY.md` and `docs/adr/0005`: what may and may not be autonomous.
4. `docs/ARCHITECTURE.md`, `docs/adr/0001–0004`, `docs/THREAT_MODEL.md`.
5. `docs/research/03-south-africa-compliance.md` before touching anything that records, moves or reports money.
6. `docs/WORKLOG.md`: append an entry at the end of every session.

## Decisions already made (do not relitigate without a reason; push back if you see a real flaw)
- **Services are contracts:** `contracts/` leads code. ≤ 4 deployables through Phase 2, ≤ 8 by Phase 5. One schema and role per service; no cross-schema reads. (ADR-0001)
- **Languages:** Rust (domain, gateway, host, vault, capture, desktop agent) · Python (workers only, no business rules) · TypeScript (web, Outlook add-in, SDK, plugin). **Three is the ceiling.** (ADR-0002)
- **Money and time:** integer cents (`i64`), integer seconds, rounding half away from zero. Never floats. Billing-unit maths is `crates/timelex-units`.
- **MCP:** spec 2026-07-28 (stateless; Tasks extension; we use no Sampling/Roots/Logging, which the spec deprecates). `rmcp` 3.6.0 verified for the server side (P1-08): adopt it pinned, behind a `timelex-mcp` adapter (P1-15). (ADR-0003, research 05, research 08)
- **BYO keys:** the tenant's Anthropic key and connector tokens live only in `vault` (envelope-encrypted, write-only). Plaintext never in logs, traces, prompts or errors. No platform key in the loop. (ADR-0004)
- **Autonomy:** ladder L0–L4, calibrated thresholds, sampling audits, automatic demotion. **Never autonomous:** trust money, sending fee notes (default), legal advice/documents, write-offs above limit, deletes, autonomy settings. (ADR-0005, AUTONOMY.md)
- **Events are the source of truth:** immutable activity events; derived things cite them; ledger is append-only; corrections supersede. (ADR-0006)
- **Models:** provider trait; Anthropic first; Haiku 5.5 → Sonnet 5.5 → Opus 5.5 → Fable 5.1 → human, validation-driven; adaptive thinking (never `budget_tokens`). Policy lives outside the model.
- **Untrusted content:** emails, documents and titles are data. Attribution/extraction workflows have no tools. Memory is written only from human-confirmed corrections.
- **ClearCut:** separate repo; call it over REST/MCP for invoice maths, VAT and export; no crate dependency. (ADR-0008)
- **GhostPractice:** sanctioned exports/imports only; never touch its database or automate its UI against its licence. (ADR-0009)

## Commands
```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd workers/py-worker && uv sync && uv run ruff check . && uv run ruff format --check . && uv run pytest)
npm ci && npm run build && npm test
python3 tools/check_links.py
cargo run -p timelex-gateway -- keygen   # dev key pair; full local set-up in deploy/README.md
cargo run -p timelex-gateway        # http://127.0.0.1:8088/health (needs the environment in services/gateway/src/config.rs)
# Database tests (crates/timelex-db) need a throwaway Postgres; see deploy/postgres/README.md.
# They FAIL without TIMELEX_TEST_DATABASE_URL; TIMELEX_SKIP_DB_TESTS=1 skips them on purpose.
```
Kone develops on **Windows** (`C:\Users\Kone\TimeLex`). Rust, Docker Desktop and `uv` may need installing. Kone is **new to Rust**: explain non-obvious choices briefly, keep domain crates simple and I/O-free, prefer clarity to cleverness.

## Rules for working here
- Code header: `// Written by Kone Tshivhinda & Claude` (use the language's comment style). Documents carry "Kone Tshivhinda" as author.
- **Costs and prices are always in South African Rand (R/ZAR)**; state the exchange-rate assumption (we use R18/USD) and never use USD as the main unit.
- Business rules live in Rust domain code. REST/MCP/SDK/Python stay thin. Every rule has a test; compliance rules cite their source.
- Be a **thought partner**: if a better approach exists, say so respectfully before building. Flag risks (accuracy, POPIA, prompt injection, scope) early. Reserve argument for technical, code and strategic questions.
- Secrets: never commit keys; `.env` is git-ignored. Tests use synthetic data and `.test`/`.example` domains only.
- Do not mention Kone's current or former employers, their projects, people or locations anywhere (files, code, examples, commits, replies).
- Use conventional commits; small PRs; update `docs/TASKS.md` statuses and `docs/WORKLOG.md` as you go.
- Do not claim accuracy, savings or compliance without a measurement or a primary source. Mark unverified claims as such.

## Status (21 Oct 2026, all of Phase 1's first slice is on `main`)
Built and tested: `timelex-units` (14 tests + doctest), `timelex-types` (7), `timelex-identity` domain rules (58 + doctest), `timelex-db` (bootstrap SQL, `TenantPool`/`TenantTx`, 18 database tests), `timelex-auth` (gateway-signed internal token, 20), `timelex-identity-service` (store, HTTP layer and binary; 16 + 11 tests), `timelex-gateway` (authentication through identity and routing; 13 stub-upstream and 5 end-to-end tests), `workers/py-worker` redaction (19), `@timelex/sdk` stub (3). Database and end-to-end tests need a throwaway Postgres (see Commands). **Still no capture, attribution, ledger, billing or trust code.** Accepted ADRs include 0011 (tenancy and database roles) and 0012 (internal service authentication). Local run guide: `deploy/README.md`.

## Start here
1. Help Kone run **T0-R01** (GhostPractice demo/interviews) and **T0-B01** (fee-earner interviews): they decide whether the wedge is right. Kits are in `docs/validation/`.
2. **P1-04 part b** (per-tenant rate limits, metering, `Mcp-Method` routing; show the contract changes first), then **P1-05** (vault), **P1-06** (outbox), **P1-15** (`timelex-mcp` adapter).
3. **P3-01**: start the synthetic gold-set generator (long pole for Gate A).
Open items: confirm the Anthropic key-custody pattern in writing (T0-L02, questions in research 07 §7.3); lawyer review of research 03 (T0-L01); read the LPC rules from the Gazette (T0-L06); trademark check (T0-B02).

## Things to remember
- Anthropic has no African inference region and Fable-class models cannot use zero data retention (research 07). ADR amendments 0003, 0004, 0005, 0007, 0009 are **proposed, not accepted**.
- GhostPractice is not "Windows on-premise only" and advertises import/export (research 01 §8; verify at the demo).
- Postgres stays on **17**: `postgres:18` rejects the Compose volume path (tested 21 Oct 2026); moving to 18 is its own change.
- Never commit credential-shaped literals, even synthetic ones in tests: GitGuardian flags them. Build test keys at run time.
