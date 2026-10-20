# mcp-host

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
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

## Contract
- **REST:** `/v1/workflows`, `/v1/runs`, `/v1/runs/{id}`, `/v1/budgets`
- **MCP tools:** `list_workflows` (R), `run_workflow` (W), `get_run` (R); long runs use the MCP Tasks extension
- **Events published:** `run.completed`, `run.needs_review`, `budget.exceeded`
- **Events consumed:** `secret.revoked`, `activity.recorded`

## Non-goals
- No Sampling (deprecated); calls the model API directly
- No free-form agents: only declared workflows
- No business rules; it orchestrates services that own them

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
