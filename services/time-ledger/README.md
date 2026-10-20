# time-ledger

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 3–4 |
| **Owns data** | `ledger` schema: append-only entries, policy decisions, review queue, hash-chained audit |

## Purpose
The record of time, and the policy engine that decides what may post automatically ([AUTONOMY.md](../../docs/AUTONOMY.md)).

## Responsibilities
- Append-only entries; corrections supersede, voids are entries; service role has INSERT and SELECT only
- Units and value via `timelex-units`; the rounding rule is stored on the entry
- Autonomy ladder L0–L4, kill switch, automatic demotion on failed audit samples
- Review queue (exceptions grouped by cause) and sampling audits
- Maker-checker for high-value or write-off actions

## Contract
- **REST:** `/v1/entries`, `/v1/entries/{id}/supersede`, `/v1/entries/{id}/void`, `/v1/policy`, `/v1/review-queue`, `/v1/audit`
- **MCP tools:** `list_entries` (R), `propose_entry` (W), `approve_entry` (W), `get_review_queue` (R)
- **Events published:** `entry.posted`, `entry.superseded`, `entry.voided`, `autonomy.demoted`
- **Events consumed:** `attribution.proposed`, `narrative.created`

## Non-goals
- Never moves trust money
- Never sends a fee note
- Never deletes

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
