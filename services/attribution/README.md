# attribution

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified; segmentation core exists |
| **Language** | Rust orchestration + `workers/py-worker` |
| **Phase** | 3 |
| **Owns data** | `attribution` schema: time blocks, candidate sets, decisions, calibration data |

## Purpose
Group events into time blocks and decide the client and matter, with evidence and calibrated confidence ([research 04](../../docs/research/04-autonomous-capture-design.md)).

## Responsibilities
- Deterministic segmentation of activity events into time blocks
- Cascade: hard keys → history → retrieval → small model → stronger model → human
- Abstention, failure reports passed to the next tier, terminal-state guarantee
- Per-tenant calibration and thresholds; weekly precision tracking

## Contract
- **REST:** `/v1/blocks`, `/v1/blocks/{id}/attribution`, `/v1/rules`
- **MCP tools:** `attribute_block` (W), `explain_attribution` (R), `list_exceptions` (R)
- **Events published:** `block.created`, `attribution.proposed`, `attribution.needs_review`
- **Events consumed:** `activity.recorded`, `matter.opened`

## Segmentation
[`crates/timelex-segmentation`](../../crates/timelex-segmentation) implements the first stage as a pure library ([ADR-0026](../../docs/adr/0026-deterministic-segmentation.md)): one tenant and one actor per call, grouping by thread → document → counterparty → lone event, anchors before background work, and clipping so blocks never overlap. Persistence into the `attribution` schema, block ids and `block.created` arrive with this service.

## Non-goals
- Does not post time (that is `time-ledger` policy)
- Model calls only via `mcp-host` workflows with no tools

## Run
The service is not runnable yet. The segmentation library is: `cargo test -p timelex-segmentation`.
See [TASKS.md](../../docs/TASKS.md) for the tasks that create the service.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
