# ADR-0018: The model loop's money and failure rules

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0003](0003-mcp-host-and-agent-runtime.md) (the host and its escalation ladder), [ADR-0004](0004-byo-keys-and-secrets-vault.md) (BYO keys and leases), [research 04 §7](../research/04-autonomous-capture-design.md) (costs) and [research 07 §3](../research/07-anthropic-terms-and-key-custody.md) (Fable and zero data retention). Implements the model-loop half of P1-13.

## Context

The host is where a firm's own money is spent and where an agent can run away. P1-13's acceptance is deliberately concrete: a mock provider drives a loop, a Rand cap aborts it, and a model the key cannot use degrades to the next rung or reaches `needs_review`. The loop must therefore be deterministic, bounded and honest about which failures can be fixed by trying a different model.

Constraints: the workspace forbids `unsafe`, runs `clippy -D warnings`, and keeps secrets out of logs, traces, prompts and errors ([ADR-0004](0004-byo-keys-and-secrets-vault.md)). Kone is new to Rust, so the money and policy rules should be plain code a reader can check, not a framework.

## Decision

1. **Money is integer micro-rand.** One Rand is 1 000 000 micro-rand, stored in a `u64`. A model call costs a few thousandths of a Rand, which is representable exactly, and two equal runs always report the same spend. Floating point is banned from the accounting.
2. **Prices are a deployment table, not tenant data.** `PriceList::published()` encodes the list rates in research 04 §7 at the R18/USD planning rate: Haiku 5.5 R1.80/R9, Sonnet 5.5 R36/R180 and Opus 5.5 R72/R360 per million input/output tokens. Cache writes are 1.25x the input rate and cache reads 0.1x, per Anthropic's multipliers. Fable has no published figure in our research, so it has no price by default.
3. **A rung exists only when it is both allowed and bounded.** The ladder is Haiku, Sonnet, Opus, Fable. Fable needs the tenant's 30-day-retention flag ([ADR-0003](0003-mcp-host-and-agent-runtime.md) amendment) **and** a configured price. Without both, Opus is the top rung and escalation goes to a human. A rung we cannot price is never attempted, because an uncapped call has no safe limit.
4. **The cap is checked before a call and after every reply.** Before a call, `spent >= cap` ends the run with `needs_review` and reason `budget_exceeded`; a zero cap makes no call at all. After a reply, `spent > cap` also ends the run, even if the reply would have been accepted: the run has already exceeded its authorization, so a human decides what to do with the output. The loop never retries past the cap.
5. **A step limit is a second, independent ceiling.** Even a run under its Rand cap stops after `max_steps` provider calls with reason `step_limit`. A cheap model in a pathological loop must not run forever.
6. **Failure class maps to one action.** The provider boundary has four errors, chosen for what happens next:
   - `ModelUnavailable` (the key is fine; this workspace cannot use this model) → try the next rung.
   - `KeyRejected` → stop with `needs_review`; every rung shares the key.
   - `Unreachable` (transport, `408`, `429`, `5xx`) → stop with `needs_review`; a rate limit or an outage is not fixed by a smaller model.
   - `Unusable` (malformed reply, no text) → stop with `needs_review`.
   Escalation past the last rung is `needs_review` with `escalated_to_human`, or `no_model_available` when rungs were only skipped as unavailable. There are no retries in the loop; backoff belongs to the workflow layer, where it can be counted against the same budget.
7. **The provider boundary carries no provider words.** The adapter maps HTTP status to the four errors and never attaches a provider message, body or status code to an error, a log or an event. Per-call records hold the tier, token counts, cache counters and Rand cost. Prompts, system messages and replies are never logged.
8. **The key is a per-run lease.** The caller leases the tenant's `anthropic_api_key` from the vault and builds the adapter with the leased `SecretValue`; dropping the adapter wipes it. The vault's wire lease (`/internal/v1/...`) lands with this first caller, in its own increment, and is audited as a lease.
9. **A library first, a listener later.** `timelex-mcp-host` is a library with no HTTP surface. The workflow contract (`/v1/workflows`, `/v1/runs`) and its routes land with P3-06, when there is a workflow to run; inventing endpoints before then would produce a contract with no users.

## Alternatives considered

- **Floating-point Rand.** Two runs of the same script would differ in the last digits, cap tests would need tolerances, and a cap breach could hide in rounding. Rejected.
- **Reading prices from the database per tenant.** Prices are provider facts, not firm settings; a stale or malicious row could set a call's cost to R0. A deployment table with a test against the research is smaller and safer. A tenant's *cap* remains tenant data.
- **Degrading on every provider error.** An invalid key would burn every rung before failing, and a rate limit would silently move a firm to a worse model. Both hide the real cause. Rejected.
- **Retrying inside the adapter.** A retry loop with its own backoff would spend money the caller's budget does not know about. Retries stay above, where the budget is. Rejected for the skeleton.
- **Guessing a Fable price from its class.** An invented rate would either overcharge the cap (stopping valid work) or undercharge it (spending past the firm's limit). Fable stays off until a rate is known and configured. Rejected.
- **Building the HTTP service and workflow schema now.** It doubles the surface with no consumer and no contract; P3-06 owns the workflow shape. Rejected.

## Consequences

- **Rates are code and must be revised when the provider changes them.** They live in one file with the research reference and a test that pins the published examples (2 000 input and 400 output tokens cost R0.0072 on Haiku).
- **A non-model `400` also degrades.** We do not read provider messages to tell them apart, by design. The run reaches `needs_review` after the ladder, which is safe but slower to diagnose; the per-call status is not retained, so a support ticket needs the run id and timestamps rather than the provider's words.
- **No prompt logging means logs cannot reconstruct a bad reply.** Debugging uses call records, token counts and the workflow's own output checks; the review queue carries the output the human needs.
- **Fable remains off by default** until both the retention flag and a price are configured, which matches the ZDR finding in research 07.
- **The lease and the MCP client side remain open.** The next increments add the vault lease (with its audit and contract tests) and the `rmcp` client probes, then the task can be marked complete.
