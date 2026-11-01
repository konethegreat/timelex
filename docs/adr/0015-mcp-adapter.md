# ADR-0015: A pinned `rmcp` adapter for MCP 2026-07-28

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0001](0001-service-boundaries-and-contract-first.md) and [ADR-0003](0003-mcp-host-and-agent-runtime.md); implements P1-15 and the server half of P1-08 ([research 08](../research/08-mcp-rust-sdk-spike.md)).

## Context

[Research 08](../research/08-mcp-rust-sdk-spike.md) proved that `rmcp` 3.6.0 can serve MCP 2026-07-28 statelessly over Streamable HTTP, including tools and the Tasks extension. It also showed that every default in its HTTP transport assumes a stateful, intranet deployment:

| Default | Why it is wrong here |
|---|---|
| `legacy_session_mode = true` | Sessions are in-process state. Any replica could serve any request only by accident. |
| `stateless_protocol_metadata_required = false` | Requests without per-request `_meta` are served as an old protocol revision instead of being rejected. |
| Empty `allowed_origins` | Origin validation is off, so a browser page on any site can reach the service (DNS rebinding). |
| Loopback `allowed_hosts` | Every real deployment host gets 403. |
| All five known protocol versions | Legacy `initialize` and old semantics stay reachable forever. |

The SDK also shipped 14 releases in the 12 weeks after the spec, and its transport behaviour is still changing. TimeLex is stateless by design ([ADR-0001](0001-service-boundaries-and-contract-first.md)), so the safe settings are not negotiable per deployment.

## Decision

1. **Pin `rmcp` exactly and keep it behind one crate.** [`crates/timelex-mcp`](../../crates/timelex-mcp) is the only crate that depends on `rmcp`, with `rmcp = "=3.6.0"`. A bump is a code change that must pass this crate's contract tests; Dependabot may open it, but it is never auto-merged.
2. **`Config` cannot represent an unsafe server.** Construction requires at least one full origin (`scheme://host[:port]`), at least one allowed `Host`, and takes a body limit (default 1 MiB) that cannot be zero. Invalid input is a typed `ConfigError`, not a warning.
3. **The transport is stateless, strict and modern-only.** `Config::transport` sets `legacy_session_mode = false`, `stateless_protocol_metadata_required = true`, the explicit origin and host lists, and the body limit. `router` serves the handler through `NeverSessionManager`, so no request depends on state left by another.
4. **Only 2026-07-28 is advertised and accepted.** `SUPPORTED_VERSIONS` has one entry, `supported_versions()` wraps it as a `Cow`, and every handler served through the adapter must return it from `ServerHandler::supported_protocol_versions`. A legacy `initialize` therefore cannot agree on a version and is refused with `-32022`.
5. **The contract tests are a Rust port of the spike.** [`tests/probes.rs`](../../crates/timelex-mcp/tests/probes.rs) starts a real server on an ephemeral loopback port for each probe and repeats the 18 checks in `spike/probe.sh`, plus the strict-metadata case and configuration validation. It asserts status, JSON-RPC error codes, headers, discovery output, task lifecycle across two independent requests, and the absence of session headers.
6. **The adapter stops at the transport.** It does not hold task state, business logic or credentials. Task rows, audit and the outbox belong to the owning service's Postgres schema, per [ADR-0003](0003-mcp-host-and-agent-runtime.md).

## Alternatives considered

- **Hand-write the JSON-RPC and Streamable HTTP layer.** The spike showed the SDK matches the spec on all 18 probes. Re-implementing it would add the transport and the Tasks extension to our security surface for no behavioural gain. Rejected.
- **Use `rmcp` defaults and fix deployments with network policy.** Origin validation is a spec MUST and network policy does not stop a same-network browser. Rejected.
- **Allow every known version for compatibility.** TimeLex has no legacy clients; the gateway already requires `MCP-Protocol-Version: 2026-07-28` ([ADR-0013](0013-gateway-limits-and-metering.md)). Accepting old revisions would only widen the surface. Rejected.
- **Set the version list in `StreamableHttpServerConfig`.** Version negotiation reads the handler, not the transport; the adapter therefore exposes a helper and documents the override. A new handler that forgets it is caught by a contract test once it is served through `router`.

## Consequences

- **A handler must opt into the version list.** `router` cannot enforce it because `rmcp` asks the handler. The adapter's own test handler proves the wiring; future services copy it, and their probes will catch a missing override.
- **Origins and hosts become deployment settings.** Each service's binary must read them from configuration; an empty list fails at startup rather than serving unvalidated traffic.
- **The contract tests pin SDK behaviour, not just ours.** Probe 18 records that the legacy path reports its rejection over SSE with HTTP 200 and `-32022`; the test asserts the rejection and the absence of a negotiated version, not the incidental status. A future SDK change that starts negotiating an old version fails the test.
- **Tasks are not durable yet.** The test handler keeps task state in memory to prove the protocol. A real service must store tasks in Postgres before it advertises the Tasks extension.
- **Gateway aggregation is still open.** [ADR-0013](0013-gateway-limits-and-metering.md)'s `tools/list`, SSE and task-id routing problems are not solved by this adapter; they belong to the service that serves those tools and to the gateway.
- **No automatic dependency bump.** Someone must read the `rmcp` changelog and watch the contract tests on every release.
