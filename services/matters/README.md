# matters

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 3 (minimal), 6 |
| **Owns data** | `matters` schema: clients, matters, contacts, aliases, rate cards, conflict index |

## Purpose
Who the clients are, what the matters are, and what they cost.

## Responsibilities
- Clients, matters, aliases and keys that make hard-key attribution possible
- Rate cards, discounts and billing arrangements
- Conflict checks (Phase 6)
- Import from CSV and, later, practice-system adapters ([ADR-0009](../../docs/adr/0009-ghostpractice-interop-strategy.md))

## Contract
- **REST:** `/v1/clients`, `/v1/matters`, `/v1/rate-cards`, `/v1/conflicts/check`
- **MCP tools:** `search_matters` (R), `check_conflicts` (R)
- **Events published:** `matter.opened`, `matter.closed`
- **Events consumed:** none

## Non-goals
- Not a document store (Phase 6 `docs`)
- FICA workflows are Phase 6

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
