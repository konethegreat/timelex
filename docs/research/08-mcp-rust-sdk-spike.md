# 08 · P1-08: Rust MCP SDK versus the 2026-07-28 spec

_Author: Kone Tshivhinda · Spike run 20 Oct 2026 · Written by Kone Tshivhinda & Claude_
Code and raw output: [spikes/p1-08-rmcp/](spikes/p1-08-rmcp/) (`results.txt`). Toolchain: Rust 1.99.1, Windows 11. Crate: [`rmcp` 3.6.0](https://crates.io/crates/rmcp), published 20 Oct 2026.

## 1. Verdict
**Adopt `rmcp` for the server side, pinned to an exact version, behind our own thin adapter crate. Do not write our own protocol layer.** The earlier worry (research 05, from ClearCut) that the Rust SDK still spoke protocol 2025-06-18 does **not** hold for 3.6.0.

Why not our own layer: the SDK already implements the awkward parts (per-request `_meta`, header mirroring and mismatch errors, version errors, Tasks, legacy fallback), and a hand-written version would copy them without the tests. Why an adapter: the crate shipped 14 releases in the 12 weeks after the spec, renamed types between versions (`Content` is now `ContentBlock`; `stateful_mode` became `legacy_session_mode`), and its defaults are unsafe for a public gateway (§4).

## 2. What we checked first (primary sources)
- Spec pages read on 20 Oct 2026: [Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http), [Versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning), [Basic: `_meta`, errors, statelessness](https://modelcontextprotocol.io/specification/2026-07-28/basic/index), [Deprecated registry](https://modelcontextprotocol.io/specification/2026-07-28/deprecated), [Tasks extension overview](https://modelcontextprotocol.io/extensions/tasks/overview).
- Crate facts: crates.io lists 3.6.0 as newest (20 Oct 2026); the crate's own `CHANGELOG.md` shows **3.0.0 on 28 Jul 2026** (spec day), `LATEST` moved to 2026-07-28 in **3.5.0 (27 Sep)**, and 3.5.1 added a 2026-07-28 fix. The README states it "implements the stable MCP 2026-07-28 specification while remaining fully compatible with the 2025-11-25 release and earlier versions".
- Spec details that matter to us: no `initialize` and no sessions; every request carries `io.modelcontextprotocol/protocolVersion` and `clientCapabilities` (both required) in `_meta`; Streamable HTTP is POST only (GET/DELETE are 405); required headers `MCP-Protocol-Version`, `Mcp-Method`, and `Mcp-Name` for `tools/call`, `resources/read`, `prompts/get`; errors `-32020 HeaderMismatch`, `-32021 MissingRequiredClientCapability`, `-32022 UnsupportedProtocolVersion` (all HTTP 400); servers MUST implement `server/discover`; **Origin must be validated (403 if present and invalid)**.
- **Correction to research 05:** Roots, Sampling and Logging are *deprecated* (earliest removal: first revision on or after 2027-07-28), not removed. Multi Round-Trip Requests still carry sampling, elicitation and roots as input requests, so "we do not use Sampling" remains a design choice, not a protocol fact. Dynamic Client Registration is also deprecated. The HTTP+SSE transport is deprecated.

## 3. What the spike did
A 130-line server (`src/main.rs`): hand-written `ServerHandler` (no macros) with two tools (`echo`, and `slow_job`, which returns a Task), `StreamableHttpService` with `NeverSessionManager` and `legacy_session_mode = false`, run twice (default, and with `stateless_protocol_metadata_required = true`). Then 18 raw `curl` probes, each written from the spec, not from the SDK's docs.

## 4. Results
| # | Probe | Spec expects | Result |
|---|---|---|---|
| 1 | `server/discover` | Result with supported versions and capabilities | **Pass.** Lists `2024-11-05` to `2026-07-28`, `tools` and the tasks extension, `serverInfo` in `_meta` |
| 2 | `tools/list` (modern `_meta`) | Tool list, `resultType: "complete"` | **Pass** |
| 3 | `tools/call` echo | Result | **Pass** |
| 4 | `tools/call` returning a task, client declared the tasks extension | `resultType: "task"`, `taskId`, status, `ttlMs`, `pollIntervalMs` | **Pass** |
| 5 | Same call **without** declaring tasks | 400, `-32021`, required capability listed | **Pass.** The SDK blocks a task result for a client that did not opt in, even though my handler tried to return one |
| 6 | `tasks/get` immediately, then 3 s later in a **new request** | `working` then `completed` with the result; no connection state | **Pass** (needs `Mcp-Name: <taskId>`; see below) |
| 7 | Missing `_meta` | 400, `-32602` | **Pass** (even in default config) |
| 8, 9, 10 | `Mcp-Name` mismatch; `Mcp-Method` mismatch; `Mcp-Method` missing | 400, `-32020` | **Pass** |
| 11 | Unsupported version `1900-01-01` | 400, `-32022`, `data.supported` | **Pass** |
| 12 | Header says 2026-07-28, body `_meta` says 2025-11-25 | 400, `-32020` | **Pass** |
| 13, 14 | GET and DELETE | 405 | **Pass** (`allow: POST`) |
| 15 | `Mcp-Session-Id` on a modern request | Ignore it; never mint or echo one | **Pass** |
| 16 | `Origin: https://evil.example` | **403** (MUST) | **Fail by default.** Returned 200. Origin checking is off unless `allowed_origins` is set |
| 17 | `Host: timelex.example` | (not in spec) | 403 by default: only loopback hosts are allowed until `allowed_hosts` is set. Safe default, but a deployed gateway must configure it |
| 18 | Legacy `initialize` (2025-11-25) | Dual-era servers MAY answer | Answered statelessly (200) |
| extra | `tools/list` with **no headers and no `_meta`** | Spec: reject | **Default config serves it as legacy 2025-03-26** (200). With `stateless_protocol_metadata_required = true` it returns 400 `-32020`. Enable this flag |

**Not tested** (so not claimed): the MCP **client** side (our `mcp-host` calling third-party servers), `tasks/update`, `tasks/cancel`, multi round-trip `input_required`, `subscriptions/listen`, OAuth (`auth` feature), the `#[tool]` macros, `json_response` mode, load, TLS, and OpenTelemetry trace propagation. Tasks state in the spike lived in memory; a real service keeps it in Postgres (the SDK does not decide that).

**Unverified detail:** the SDK demands `Mcp-Name: <taskId>` on `tasks/get`, `tasks/update` and `tasks/cancel`, citing SEP-2663. The core transport page lists `Mcp-Name` only for `tools/call`, `resources/read` and `prompts/get`, and we could not open the Tasks specification text to confirm. Follow the SDK, and check the extension spec before writing the gateway's header routing.

## 5. What we must configure (the defaults are not production-safe)
| Setting | Default in 3.6.0 | What we set | Why |
|---|---|---|---|
| `legacy_session_mode` | `true` | `false` | ADR-0001: stateless, any instance serves any request |
| `stateless_protocol_metadata_required` | `false` | `true` | Reject requests without per-request metadata, as the spec requires |
| `allowed_origins` | empty, meaning **no Origin validation** | explicit list | Spec MUST on DNS-rebinding defence; probe 16 |
| `allowed_hosts` | loopback only | our hostnames | Otherwise every real request gets 403 |
| `max_request_body_bytes` | SDK default | set deliberately | Memory limits at the gateway |
| `supported_protocol_versions` | all of 2024-11-05 to 2026-07-28 | `2026-07-28` only (our own clients are modern) | Smaller attack surface; the SDK documents this combination |
| Session manager | `LocalSessionManager` | `NeverSessionManager` | No in-process sessions |

## 6. Consequences for the plan
1. **P1-08 acceptance is met** for the server side: `tools/list` and `tools/call` over stateless HTTP work, with Tasks. Mark it done for the server; the **client** half moves to P1-13 (`mcp-host` skeleton), where it will be tested for real.
2. Build a `timelex-mcp` adapter crate that owns the config in §5, converts our contract types to SDK types, and contains a **Rust port of `probe.sh` as contract tests**. A version bump of `rmcp` must pass those tests in CI before it merges. This also answers "should we write our own thin layer": yes, but only the thin *adapter*, not the protocol.
3. Pin `rmcp = "=3.x.y"` and review its changelog on each Dependabot bump; do not auto-merge.
4. **Tell ClearCut**: its note that `rmcp` 3.5.1 spoke `2025-06-18` does not match the changelog or this spike. We did not test 3.5.1, so we cannot say what it observed (it may have been a client default or a feature combination).
5. Tasks need a Postgres-backed task store and our own TTL rules ([ADR-0003](../adr/0003-mcp-host-and-agent-runtime.md) already says state is a row).
