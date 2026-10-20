# ADR-0008: Relationship to ClearCut

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude

## Context
[ClearCut](https://github.com/konethegreat/clearcut) is Kone's Apache-2.0 Rust project for invoices: money and VAT maths, lifecycle, event model, cited narration with a number guard, and export (PDF, Excel, Word, CSV, JSON, text/Markdown/HTML), exposed as REST, MCP and a Claude plugin. As of 20 Oct 2026 its core is built (181 tests) while ingestion, persistence and export are not. TimeLex produces the time, WIP and pro-forma that become fee notes.

## Decision
1. **Separate repositories, separate release cycles, one API contract.** TimeLex's `billing` service calls ClearCut through its REST API or MCP tools (via a thin client in `services/billing`). **No Rust crate dependency** on ClearCut in Phase 1–5.
2. **ClearCut owns:** invoice and credit-note maths, VAT categories and thresholds, invoice lifecycle, export rendering, e-invoicing readiness.
3. **TimeLex owns:** capture, attribution, time entries, rate cards, WIP, pro-forma assembly, matter and client context, autonomy policy.
4. **Hand-off object:** a `ProFormaRequest` (lines in integer cents and thousandth-quantities, tax categories, matter reference, narrative lines with citations) defined in `contracts/` and mapped to ClearCut's invoice input. The mapping is contract-tested against ClearCut's published OpenAPI.
5. **Shared conventions, not shared code:** integer minor units, half-away-from-zero rounding, ADR style, "Written by Kone Tshivhinda & Claude" headers, ZAR-first costs, Apache-2.0.
6. **Shared concept to extract:** the **number guard and cited-narration** pattern is needed in both. When TimeLex's narrative service is built (Phase 3) we compare implementations and, if they match, extract an `evidence-narration` library into its own repo that both depend on. Until then each keeps its own.
7. **Failure isolation:** if ClearCut is unavailable, pro-forma generation queues; time capture and posting continue.

## Alternatives considered
- *Merge the repos.* Defeats the no-monolith goal and mixes release cadences.
- *Depend on `clearcut-core` as a git crate.* Fast, but couples versions and hides the API as the contract.
- *Re-implement invoicing in TimeLex.* Duplicates tested VAT code.

## Consequences
- ClearCut's own roadmap (persistence, export) gates TimeLex's fee-note export. Mitigation: Phase 5 ships CSV/JSON pro-forma first, then ClearCut-rendered PDF/Excel when available.
- ClearCut's MCP currently speaks protocol 2025-06-18; both projects move to 2026-07-28 together (P1-08, ClearCut P1-06).
