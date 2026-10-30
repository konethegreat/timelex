# ADR-0013: Gateway limits, metering and MCP header routing

Status: Accepted · Date: 2026-10-30 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0012](0012-internal-service-authentication.md) and [ADR-0003](0003-mcp-host-and-agent-runtime.md); implements P1-04 part b.

## Context

Part a authenticates every caller through identity, signs an internal token and routes three identity operations. Part b must:

- protect the platform from one firm exhausting shared services;
- record use per firm without turning the gateway into another data store;
- route MCP requests by header, because the 2026-07-28 specification requires `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` and lets a gateway route without parsing bodies.

The gateway owns no data and is deliberately database-free. The outbox (P1-06) and OpenTelemetry (P1-10) do not exist yet. The MCP adapter (P1-15) owns the protocol details.

## Decision

1. **Rate limits are token buckets, one per firm, checked after authentication and before the request body is read.** Defaults are 600 requests a minute with a burst of 120; both are environment settings. The platform operator gets its own bucket at 60 a minute. A caller over budget gets 429 with `Retry-After` in whole seconds.
   - **Keying after authentication is deliberate.** The tenant id is in the API-key prefix, but an unauthenticated caller can write any prefix. A pre-auth bucket keyed on it would let one caller burn another firm's budget.
   - **The store is in process.** Buckets are lost on restart and each replica has its own. The effective limit therefore multiplies by the number of replicas until a shared store lands. Phase 1 runs one gateway process.
   - Bucket count is bounded by real tenants, because only authenticated principals create a bucket.

2. **Metering records request metadata only.** One record per handled request: time, caller kind, tenant, user and key ids, route label, target service, method, status, duration, bytes in and out, whether it was rate limited, and the MCP tool name when present. Never the body, the credential or personal data.
   - Interim sinks are one JSON line on stdout and `GET /internal/usage` on a separate loopback listener, showing totals per firm since start.
   - **No Postgres table in the gateway.** The gateway owns no data; P1-06 (outbox) and P1-10 (OpenTelemetry) become the durable path. This is request metering, not model spend. Model spend is metered by `mcp-host` (P3-12).

3. **MCP requests are routed by header, never by parsing the body.** `POST /mcp` requires `MCP-Protocol-Version: 2026-07-28` and `Mcp-Method`; `tools/call` also requires `Mcp-Name`. The tool name is looked up in `contracts/mcp/routes.json` and the request is forwarded unchanged to the owning service with an internal token addressed to it.
   - Only `tools/call` is routed in this version. `server/discover`, `tools/list` and `tasks/*` answer 501 until P1-15.
   - The service re-checks the headers against the body and enforces `Origin` (threat T21). The gateway does not duplicate the protocol layer.
   - `tasks/*` cannot be routed by `Mcp-Name` yet because that header carries the task id, not a tool name. P1-15 must give task ids a service prefix or add a lookup.

4. **Configuration changes.** `TIMELEX_SERVICE_URLS` (for example `identity=http://127.0.0.1:8081`) replaces `TIMELEX_IDENTITY_URL`, so every MCP target has one map. `TIMELEX_RATE_LIMIT_PER_MINUTE` and `TIMELEX_RATE_LIMIT_BURST` set the firm budget. `TIMELEX_GATEWAY_INTERNAL_ADDR` sets the loopback listener. Bad values fail at startup.

## Alternatives considered

- **Pre-authentication limits keyed on the key prefix.** Cheap, but spoofable into a denial-of-service tool against a chosen firm. Rejected.
- **Postgres-backed metering now.** Durable and queryable, but adds a schema, migrations and a database role to a component that owns no data, before the outbox exists. Rejected for Phase 1.
- **Redis or another shared rate-limit store now.** Correct for multiple replicas, but adds a deployable before there is a second replica. Deferred.
- **Full MCP aggregation now.** Needs the `rmcp` adapter and a task-id routing scheme. Deferred to P1-15.

## Consequences

- The gateway stays database-free and cheap to run.
- Limits do not protect identity from invalid-key floods, because authentication happens first. Mitigation belongs at identity or the edge (per-IP limits) and is recorded as a known gap, not built here.
- Usage totals reset on restart; the response carries `since` and uptime so nobody mistakes them for billing records.
- `contracts/mcp/routes.json` must change with `contracts/mcp/tools.md`; a gateway test fails when they drift.
- 501 is honest: an unrouted method is refused, never silently proxied to the wrong service.
- Streaming responses (SSE) are not supported yet; the gateway buffers. P1-15 revisits the transport.
