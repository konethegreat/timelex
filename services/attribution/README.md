# attribution

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified; segmentation, stages 0–1, retrieval and model stages exist as libraries; HTTP and persistence pending |
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

## Retrieval
[`workers/py-worker`](../../workers/py-worker) holds the retrieval stage ([ADR-0029](../../docs/adr/0029-retrieval-local-embeddings-per-tenant-namespace.md)): deterministic local token-hash vectors, cosine search, and one namespace per tenant. The worker keeps vectors only, never text, and refuses a namespace file that names another tenant. Its `index` and `search` contract is in [`contracts/attribution`](../../contracts/attribution). The service will call it with the tenant from request context; it does not decide whether a candidate is good enough to post.

## Model stages
[`timelex-attribution-service`](.) carries stages 3–4 ([ADR-0030](../../docs/adr/0030-model-stages-propose-only-offered-matters.md)). It supplies three things to the existing `mcp-host` ladder rather than walking one of its own: the prompt, a validator that accepts only a candidate that was offered, and a failure report. A reply must be JSON with either `{"matterId":"<id>"}` for exactly one candidate or `{"abstain":true}`; prose, extra fields and invented matter ids are failures and escalate. An abstain escalates too, because it is the low-confidence signal research 04 acts on. Every input ends in `Proposed` or `Review`, and a proposal still passes the two-key checks before a post. Each later rung reads the earlier failures, and the outcome returns every call with its tier, usage and Rand cost plus the total spend. With no candidates the model is never called. The activity is framed as data and a closing tag is neutralised; the adversarial corpus is P3-10.

## Non-goals
- Does not post time (that is `time-ledger` policy)
- Model calls only via `mcp-host` workflows with no tools

## Run
`cargo test -p timelex-attribution` (stages 0–1), `cargo test -p timelex-attribution-service`
(model stages) and `cargo test -p timelex-segmentation`. The HTTP service is not runnable yet.
See [TASKS.md](../../docs/TASKS.md) for the tasks that create the service.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
