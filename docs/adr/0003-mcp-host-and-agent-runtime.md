# ADR-0003: MCP host and agent runtime

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Builds on [research 05](../research/05-mcp-host-and-byo-keys.md).

## Context
TimeLex needs a server-side component that runs agent loops (attribution, narration, billing assistants) against the tenant's model account and tools, and also lets customers' own agents call TimeLex. The MCP 2026-07-28 spec is final and stateless; Sampling, Roots and Logging are deprecated.

## Decision
1. **`services/mcp-host`** (Rust) is the **inward** host: it holds MCP clients to our services and to third-party MCP servers, owns the model loop, and enforces policy.
2. The **gateway** is the **outward** face: it aggregates our services' MCP catalogues behind OAuth 2.1/OIDC and API keys.
3. **Stateless.** No in-process sessions. A "conversation" or job is a row (`task_id`) with its context pack stored by reference; any instance resumes it.
4. **Model calls are direct** to the provider with the tenant's key fetched from the vault per call. We do not use MCP Sampling.
5. **Provider trait** (`ModelProvider`) with Anthropic first; budgets, retries, tier routing and logging sit above it.
6. **Workflows, not free agents.** Each workflow (e.g. `attribute_block`, `narrate_entry`, `prepare_prebill`) declares: tools allowed, schema of output, max steps, max Rand, escalation ladder, and terminal states (`done` or `needs_review`). No workflow can call a tool it did not declare.
7. **Approvals** use Multi Round-Trip Requests; long jobs use the **Tasks** extension.
8. **Memory** is a per-tenant store written only from human-confirmed corrections; never from text read out of captured content.
9. **Observability:** OpenTelemetry traces with per-call model, tier, tokens, cost, cache hits, escalation reason.

## Alternatives considered
- *Use the Claude Agent SDK / Managed Agents as the runtime.* Faster to start; weaker control over per-tenant key custody, residency, policy-outside-the-model and multi-provider. **Spike T0-A01** at the start of Phase 4; adopt only if the control gaps are acceptable.
- *Let outer agents (Claude Desktop) be the only hosts.* Simple, but cannot run autonomously on a schedule and cannot enforce our policy.
- *Merge host into gateway.* Different scaling and blast radius; keep separate.

## Consequences
- We own prompt-injection defence at the workflow layer ([THREAT_MODEL.md](../THREAT_MODEL.md)).
- The SDK choice (`rmcp` vs thin adapter) is gated by task **P1-08**.
- ClearCut's separate-process MCP decision is respected at the *host/gateway* level; services bind MCP on a distinct listener (ADR-0001 §3) rather than separate binaries, which differs from ClearCut and is revisited if blast-radius concerns appear.
