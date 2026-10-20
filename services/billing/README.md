# billing

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 5 |
| **Owns data** | `billing` schema: WIP, pro-formas, certifications, exports |

## Purpose
From posted time to a certified pro-forma and fee-note data, using ClearCut for invoice maths, VAT and exports ([ADR-0008](../../docs/adr/0008-clearcut-relationship.md)).

## Responsibilities
- WIP and ageing from the ledger
- Pro-forma assembly with narratives and citations
- Certification workflow (fee earner or partner)
- `ProFormaRequest` → ClearCut REST/MCP client with contract tests
- Exports: CSV/JSON; GhostPractice-ready import file if the licence allows (T5-L01)

## Contract
- **REST:** `/v1/wip`, `/v1/proformas`, `/v1/proformas/{id}/certify`, `/v1/exports`
- **MCP tools:** `get_wip` (R), `prepare_proforma` (W), `certify_proforma` (W)
- **Events published:** `proforma.prepared`, `proforma.certified`
- **Events consumed:** `entry.posted`

## Non-goals
- Does not send fee notes by default (opt-in per client)
- Does not re-implement VAT (ClearCut owns it)

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
