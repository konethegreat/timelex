# MCP tool catalogue (draft)

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Status: **design draft, nothing implemented.** Every service exposes its tools under `/mcp` (Streamable HTTP, stateless, MCP 2026-07-28) and external agents reach them through the gateway ([ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md), [ADR-0003](../../docs/adr/0003-mcp-host-and-agent-runtime.md)).

Rules for every tool:
1. **Annotations:** read-only tools are marked read-only; mutating tools are marked mutating and **require an idempotency key**.
2. **Structured output only:** integer cents, integer seconds, enums, ISO 8601 UTC timestamps. No prose parsing.
3. **Tenant** comes from the caller's credentials, never from a tool argument.
4. **Untrusted text** (email subjects, document names) is returned in clearly typed fields and never as instructions.
5. **Approvals** use Multi Round-Trip Requests (`InputRequiredResult`), so a human or policy can answer mid-call.
6. Tools that move money or send to clients are **not exposed to agents** unless a tenant policy explicitly allows it, and trust-money tools are never exposed ([AUTONOMY.md](../../docs/AUTONOMY.md)).

| Service | Tool | R/W | Purpose |
|---|---|---|---|
| identity | `list_users` | R | List users and roles |
| identity | `create_api_key` | W | Issue a scoped API key (admin only) |
| vault | `list_secrets` | R | Metadata only (kind, last four, last used); never values |
| vault | `test_secret` | R | Check a stored key still works |
| vault | `revoke_secret` | W | Revoke a stored secret |
| mcp-host | `list_workflows` | R | Available workflows and their budgets |
| mcp-host | `run_workflow` | W | Start a workflow run (returns a task id) |
| mcp-host | `get_run` | R | Status, cost, terminal state (`done` / `needs_review`) |
| capture-hub | `list_connectors` | R | Connectors and subscription health |
| capture-hub | `start_backfill` | W | Back-fill a mailbox/calendar range (long task) |
| capture-hub | `quick_capture` | W | Record a manual activity event |
| attribution | `attribute_block` | W | Propose a client/matter for a time block |
| attribution | `explain_attribution` | R | Evidence and decision record |
| attribution | `list_exceptions` | R | Blocks needing a human, grouped by cause |
| narrative | `narrate_entry` | W | Produce a cited narrative for a time block |
| narrative | `check_narrative` | R | Run the number guard on a given text |
| time-ledger | `list_entries` | R | Query posted time |
| time-ledger | `propose_entry` | W | Draft an entry (policy decides posting) |
| time-ledger | `approve_entry` | W | Human approval of a drafted entry |
| time-ledger | `get_review_queue` | R | Pending exceptions |
| matters | `search_matters` | R | Find clients/matters |
| matters | `check_conflicts` | R | Conflict check for a party name |
| billing | `get_wip` | R | Work in progress by client/matter |
| billing | `prepare_proforma` | W | Assemble a pro-forma (calls ClearCut for maths/VAT) |
| billing | `certify_proforma` | W | Fee-earner certification |
| trust | `get_trust_balance` | R | Read-only; **no mutating trust tools are exposed** |
