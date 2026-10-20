# ADR-0009: GhostPractice interoperability and migration

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Builds on [research 01](../research/01-ghostpractice-teardown.md).

## Context
GhostPractice is a Windows on-premise system with, per a listing, no public API. Firms will not switch on day one. The main barrier is switching cost, not features.

## Decision
1. **Three modes**, in order of build:
   - **Shadow mode:** TimeLex captures and prepares time; the firm imports it into GhostPractice using a **sanctioned path** (CSV/report import if GhostPractice offers one, or a clearly user-driven copy/paste-ready export). Nothing writes into GhostPractice automatically.
   - **Side-by-side:** TimeLex imports matters, clients and rates from sanctioned **exports** so attribution knows the firm's matters.
   - **Replace:** back-office modules (billing, then trust) once Tier A/B are trusted and Tier C is reviewed.
2. **Never** read or write GhostPractice's database directly, decompile it, or automate its UI in ways its licence forbids. Check the licence first (task **T5-L01**); ask the vendor for a partner/export arrangement.
3. **Adapter isolation:** all GhostPractice-specific logic lives in `connectors/ghostpractice-*` behind a `PracticeSystemAdapter` interface so Clio, LEAP, LegalSuite and others can follow.
4. **Import tooling** is a product feature: dry-run reports, reconciliation of totals before and after, and a rollback plan.
5. **Honesty in docs:** we do not claim GhostPractice integration until a firm uses it in production.

## Alternatives considered
- *Direct database integration.* Fast, fragile, probably forbidden.
- *UI automation (RPA).* Brittle; likely licence issues; only with the firm's written consent and vendor tolerance.
- *Ignore GhostPractice and target new firms.* Smaller addressable market in the near term; kept as a parallel path (Clio/other adapters).

## Consequences
- The first pilot depends on finding one firm and one working export path. If none exists, shadow mode degrades to "ready-to-key" exports, which still saves the hard part (capture and narrative).
- This decision may be revised after the demo and interviews (T0-R01).
