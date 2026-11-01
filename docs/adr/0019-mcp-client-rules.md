# ADR-0019: The host's MCP client rules

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0003](0003-mcp-host-and-agent-runtime.md), [ADR-0015](0015-mcp-adapter.md) and [ADR-0018](0018-model-loop-money-and-failure-rules.md); implements the client half of P1-13.

## Context

[ADR-0003](0003-mcp-host-and-agent-runtime.md) makes `mcp-host` the inward MCP client: it will call TimeLex's own services, and later third-party servers, on behalf of a workflow. [ADR-0015](0015-mcp-adapter.md) pinned the server half of `rmcp` 3.6.0 and the 2026-07-28 revision, but the SDK's client defaults are equally unsafe:

| Default | Why it is wrong here |
|---|---|
| `ClientLifecycleMode::Auto` | Probes with `server/discover`, then silently falls back to the legacy `initialize` handshake after 10 seconds or on a legacy error. A server is never allowed to negotiate TimeLex down. |
| `reinit_on_expired_session = true` | Re-runs the handshake on a 404. TimeLex servers are stateless and never issue a session, so a 404 is a routing error and re-handshaking hides it. |
| Any `http://` endpoint | A bearer token and model-tool arguments would cross the network in clear text. |
| No bearer handling | The credential would end up in call sites and, easily, in logs. |

The host also needs a stable error surface: [ADR-0018](0018-model-loop-money-and-failure-rules.md) established that a provider's words never become an error value, and a remote server's message deserves the same treatment.

## Decision

1. **Targets are validated before a connection exists.** `ClientTarget::new` accepts an absolute URL and requires `https`, except that plain `http` is allowed for a loopback host (`127.0.0.0/8`, `::1` or `localhost`). A URL with a username, password or fragment is refused. Every failure is a typed `TargetError`.
2. **Discovery only, pinned version only.** `McpClient::connect` uses `ClientLifecycleMode::Discover { preferred_versions: [2026-07-28] }`. It never sends `initialize` and never falls back. A legacy-only server fails startup; a server that discovers but offers only older revisions fails as `Protocol`.
3. **Stateless, no session recovery.** The transport sets `allow_stateless = true` and `reinit_on_expired_session = false`. A 404 is surfaced, not papered over.
4. **The bearer token has one route and one representation.** `with_bearer_token` validates that the token is non-empty, at most 8192 printable ASCII characters, and stores it in a zeroizing buffer inside `ClientTarget`. It reaches the wire only as `Authorization: Bearer …`; `Debug` redacts it; dropping the target wipes it. A URL never carries a credential.
5. **One call, one result.** `call_tool` uses `call_tool_once` and refuses both `input_required` (the host does not provide MRTR input yet) and a returned task (the host does not poll `tasks/get` yet). A workflow needs a finished result or a reason it has none.
6. **Errors are closed and quiet.** `ClientError` has seven variants: `Startup`, `Protocol`, `Transport`, `Rpc { code }`, `InputRequired`, `Task` and `Closed`. Only the numeric JSON-RPC code is kept; the server's message, body and URL never enter the value.
7. **The client is tested against a real server, not just mocks.** [`tests/client.rs`](../../crates/timelex-mcp/tests/client.rs) connects to the in-repo stateless server over loopback and checks discovery, `tools/list`, `tools/call`, no session header, and the bearer header with no token in any other header. Stub servers prove that a legacy-only peer is refused without an `initialize` and that an older-only peer fails the version pin.

## Alternatives considered

- **`Auto` lifecycle for third-party compatibility.** It is the path to a silent downgrade and a second protocol era in one process. Rejected. When the first third-party connector lands, it brings its own explicit compatibility decision; the default stays pinned.
- **Allow plain HTTP on private networks.** `10/8` and `172.16/12` are still networks the bearer token should not cross unprotected, and "private" is an assumption, not a property of the URL. Rejected.
- **Carry the server's JSON-RPC message in the error.** Useful in isolation, but it is remote text that would reach logs and possibly a model prompt. The code is enough to branch; a human reads the server's own logs for the words. Rejected.
- **Support MRTR and tasks now.** Both need workflow state and a place to ask a human, which P3-06 owns. Rejected for this increment; the refusal is explicit and tested by construction.
- **Use `rmcp`'s block-on-`call_tool` MRTR helper.** It would silently turn a server request for input into an error from an empty local handler, which is harder to reason about than an explicit refusal.

## Consequences

- **Older servers are unusable until a compatibility decision is made.** That is intentional for Phase 1, because every server the host calls is ours. The first external connector must revisit this ADR rather than quietly switching to `Auto`.
- **No MRTR or task polling.** Workflows requiring approval or long jobs wait for P3-06/P2-11. The client's `InputRequired` and `Task` variants make the gap visible instead of hanging.
- **No per-call deadline yet.** `mcp-host`'s loop owns budgets and step limits ([ADR-0018](0018-model-loop-money-and-failure-rules.md)); a hung transport is bounded by the workflow's overall deadline when that lands. `control_request_timeout` stays at the SDK default of five seconds.
- **TLS trust is the platform verifier.** Certificate pinning is not implemented; it would be a per-target setting when third-party servers arrive.
- **The adapter now holds a credential type.** ADR-0015 §6 said the adapter does not hold credentials; this ADR narrows that to "holds a caller-supplied bearer token for the lifetime of a target, in a zeroizing redacted wrapper, and nothing else." The vault remains the only place secrets are stored.
