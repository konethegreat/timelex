# narrative

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 3 |
| **Owns data** | `narrative` schema: templates, client styles, generated narratives with evidence |

## Purpose
Write fee-ready narratives that cite evidence and cannot invent numbers.

## Responsibilities
- Template-first narration; optional LLM smoothing via `mcp-host`
- Per-client narrative style and prohibited-wording rules
- Number guard: any numeral or date not present in cited events is rejected, falling back to the template
- Redaction check before a narrative leaves the platform

## Contract
- **REST:** `/v1/narratives`
- **MCP tools:** `narrate_entry` (W), `check_narrative` (R)
- **Events published:** `narrative.created`
- **Events consumed:** `attribution.proposed`

## Non-goals
- Shares the *pattern* with ClearCut; extraction into a common library happens only when both implementations match (ADR-0008)

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
