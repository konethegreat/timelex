# mcp-host

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 model loop implemented; MCP clients, lease and HTTP routes pending |
| **Language** | Rust |
| **Phase** | 1–4 |
| **Owns data** | `host` schema: workflow definitions, runs, context-pack references, budgets, memory (confirmed corrections only) |

## Purpose
Server-side MCP host and agent runtime ([ADR-0003](../../docs/adr/0003-mcp-host-and-agent-runtime.md)). Runs declared workflows against the tenant's own model account.

## Responsibilities
- Hold MCP clients to our services and to allow-listed third-party MCP servers
- Model loop behind a `ModelProvider` trait (Anthropic first) using the tenant's key from `vault`
- Workflows declare allowed tools, output schema, max steps, max Rand, escalation ladder and terminal states (`done` / `needs_review`)
- Haiku → Sonnet → Opus → Fable → human escalation driven by validation, disagreement and confidence
- Per-call logging of model, tier, effort, tokens, cache hits, Rand cost, escalation reason

## Implemented (P1-13 increment 1, [ADR-0018](../../docs/adr/0018-model-loop-money-and-failure-rules.md))
This is a library, not a listener yet; the workflow contract and its routes arrive with P3-06.

- **`ModelProvider`** with four errors chosen for what the loop does next: `ModelUnavailable` (try the next rung), `KeyRejected` (stop; every rung shares the key), `Unreachable` (stop; a rate limit or outage is not the key's fault) and `Unusable` (stop).
- **`AnthropicProvider`**: one `POST /v1/messages`, the leased key in `x-api-key` and never in a URL, body, error or log. `401` stops the run; `400`/`403`/`404` degrade to the next rung; `408`/`429`/`5xx` and transport failures stop it. No retries here; backoff belongs where the budget is.
- **Money in integer micro-rand** at the rates in [research 04 §7](../../docs/research/04-autonomous-capture-design.md): Haiku R1.80/R9, Sonnet R36/R180, Opus R72/R360 per million input/output tokens, with cache reads at 0.1x and cache writes at 1.25x the input rate. Fable has no published rate, so it is off until a deployment configures one.
- **The ladder** is Haiku → Sonnet → Opus → Fable. Fable needs the tenant's 30-day-retention flag *and* a price; without both, Opus is the top rung and escalation reaches `needs_review`.
- **Budgets**: the cap is checked before every call and after every reply. A breach ends the run with `budget_exceeded`; a zero cap makes no call. `max_steps` is a second ceiling. Every run ends `done` or `needs_review`.
- **Per-call records** (`Call`) carry the tier, outcome, token counts, cache counters and Rand cost. Prompts, system messages and replies are never logged.

### Tests
```
cargo test -p timelex-mcp-host
```
2 unit tests (money arithmetic and the published rates), 14 loop tests with a scripted mock provider (accept, escalation, unavailable-model degradation, rejected key, unreachable and unusable replies, Rand cap, zero cap, step limit) and 9 adapter tests against a local Axum stub (headers, request shape, reply shape, every error class, a provider that is not there). No test reaches the real provider.

## Contract (not served yet)
- **REST:** `/v1/workflows`, `/v1/runs`, `/v1/runs/{id}`, `/v1/budgets`
- **MCP tools:** `list_workflows` (R), `run_workflow` (W), `get_run` (R); long runs use the MCP Tasks extension
- **Events published:** `run.completed`, `run.needs_review`, `budget.exceeded`
- **Events consumed:** `secret.revoked`, `activity.recorded`

## Key path
The caller leases the tenant's `anthropic_api_key` from the vault (ADR-0004), builds an `AnthropicProvider` with the leased `SecretValue`, runs, and drops it; the value is wiped on drop and its `Debug` is redacted. The vault's wire lease (`/internal/v1/...`) lands with this first caller, with its own audit and contract tests, before any workflow can be triggered remotely.

## Next
1. Vault lease with audit and contract tests, and a lease-backed key source.
2. `rmcp` client probes (the MCP client side of P1-15/P1-13).
3. The `host` schema, workflow definitions, `/v1/runs` and the three events (P3-06).

## Non-goals
- No Sampling (deprecated); calls the model API directly
- No free-form agents: only declared workflows
- No business rules; it orchestrates services that own them

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
