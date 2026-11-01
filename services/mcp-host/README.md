# mcp-host

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 model loop, MCP client and vault lease implemented; HTTP workflow routes pending |
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

## MCP client (P1-13 increment 3, [ADR-0019](../../docs/adr/0019-mcp-client-rules.md))
The host's connection to a server lives in [`timelex-mcp`](../../crates/timelex-mcp/src/client.rs), the same pinned crate that serves the server half.

- `ClientTarget` validates before a connection exists: absolute URL, HTTPS except loopback (`127.0.0.0/8`, `::1`, `localhost`), and no username, password or fragment in the URL.
- Discovery only: the pinned 2026-07-28 revision, no `initialize`, no fallback. A legacy-only server fails startup; an older-only server fails as `Protocol`.
- Stateless: no session is issued or reused, and session re-initialisation is off, so a 404 is surfaced rather than hidden.
- The bearer token is validated, zeroizing, redacted in `Debug`, and sent only in `Authorization`.
- `call_tool_once` returns a finished result; `input_required` and task replies are refused until the workflow layer owns MRTR input and task polling.
- Errors are closed: `Startup`, `Protocol`, `Transport`, `Rpc { code }`, `InputRequired`, `Task`, `Closed`; the server's message never enters one.

```
cargo test -p timelex-mcp
```
32 tests: 23 server probes and 9 client tests against the in-repo stateless server and legacy/older stubs.

## Vault lease (P1-13 increment 2, [ADR-0020](../../docs/adr/0020-workload-identity-and-leases.md))
The key path is a real wire call, not an assumption that the caller has the key.

- `VaultKeySource` holds this service's own Ed25519 key and builds a workload token per lease: `iss` `timelex-mcp-host`, `aud` `vault`, kind `workload`, the firm, the initiating user and a `LeasePurpose`. The vault trusts the key explicitly; nothing is shared with the gateway.
- `LeasePurpose::ModelRun` is the only purpose. It maps to `anthropic_api_key` in both the client and the vault, so the host can ask for exactly one kind.
- `lease()` posts to `/internal/v1/secrets/anthropic_api_key/lease` with no body and parses `{ kind, value, expiresAt }`. It accepts only the kind it asked for, checks the value through `SecretValue::parse`, and never logs the body.
- `Lease` wraps the value in a zeroizing `SecretValue`, redacts it in `Debug` and carries `expiresAt`. The run must drop it at expiry or when it ends, whichever comes first; the vault keeps no copy to recall.
- `LeaseError` is closed: `Unauthorized` (this key is not trusted), `Forbidden` (wrong purpose), `NotFound` (the firm has no live key), `Unavailable` (transport, `408`, `429`, `5xx`) and `Unusable` (any other status, a malformed body or the wrong kind). The vault's words and body never enter one.

```
cargo test -p timelex-mcp-host --test vault
```
5 tests against a stub vault on loopback that verifies the token exactly as the real one does: the method, path and claims; the returned value and expiry; `Debug` redaction; every status class; malformed and mismatched bodies; an unreachable vault; and a base URL that is not HTTP.

## Contract (not served yet)
- **REST:** `/v1/workflows`, `/v1/runs`, `/v1/runs/{id}`, `/v1/budgets`
- **MCP tools:** `list_workflows` (R), `run_workflow` (W), `get_run` (R); long runs use the MCP Tasks extension
- **Events published:** `run.completed`, `run.needs_review`, `budget.exceeded`
- **Events consumed:** `secret.revoked`, `activity.recorded`

## Key path
The caller uses a `VaultKeySource`, configured with this service's own key, to lease the tenant's `anthropic_api_key` for `model_run` ([ADR-0020](../../docs/adr/0020-workload-identity-and-leases.md)). The vault verifies the workload token, records a `used` audit entry naming the service and purpose, and returns the value once with a 60-second expiry. The caller builds an `AnthropicProvider` with the leased `SecretValue`, runs, and drops it; the value is wiped on drop and its `Debug` is redacted.

## Next
1. The `host` schema, workflow definitions, `/v1/runs` and the three events (P3-06).

## Non-goals
- No Sampling (deprecated); calls the model API directly
- No free-form agents: only declared workflows
- No business rules; it orchestrates services that own them

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
