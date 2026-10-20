# trust

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified, **gated (Gate C)** |
| **Language** | Rust |
| **Phase** | 7 |
| **Owns data** | `trust` schema: business and trust ledgers, reconciliations, reports |

## Purpose
Regulated trust and business accounting aligned with the LPC Accounting Rules (Part XII). **No code until Gate C passes** ([PHASES.md](../../docs/PHASES.md)).

## Responsibilities
- Separate, distinguishable trust and business ledgers (rules 54.6, 54.8)
- Invariants: trust cover ≥ creditors, no trust creditor in debit, no commingling (54.11, 54.14.8–54.14.11)
- Receipts and payments with maker-checker and payee-bank-detail audit trail (54.14.5)
- Monthly reconciliation (54.15), LPC reports and audit pack, seven-year retention, backups (54.14.3–54.14.4)

## Contract
- **REST:** `/v1/ledgers`, `/v1/receipts`, `/v1/payments`, `/v1/reconciliations`, `/v1/reports`
- **MCP tools:** `get_trust_balance` (R) only. **No mutating trust tool is ever exposed to agents.**
- **Events published:** `trust.reconciled`, `trust.shortfall_detected`
- **Events consumed:** `proforma.certified`

## Non-goals
- Never autonomous
- May be replaced by an integration with a partner's regulated ledger (research 06 §2)

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
