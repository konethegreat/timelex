# gateway

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ P1-04 part a: authentication and routing to identity. ✅ Part b: per-firm rate limits, request metering and MCP header routing. ✅ 0.4.0: the five vault secrets routes (28 unit, 2 contract, 27 stub-upstream and 9 end-to-end tests) |
| **Language** | Rust (axum, reqwest) |
| **Phase** | 1 |
| **Owns data** | none (config only; usage totals are in memory) |

## Purpose
Edge of the platform. One front door: it authenticates the caller, then routes to a service with a short-lived signed token. Design: [ADR-0012](../../docs/adr/0012-internal-service-authentication.md), [ADR-0013](../../docs/adr/0013-gateway-limits-and-metering.md), [contracts/internal-auth.md](../../contracts/internal-auth.md).

## What it does today
- `GET /health`, `GET /v1/meta`: open.
- `POST /v1/tenants` (operator key), `POST /v1/api-keys`, `POST /v1/api-keys/{keyId}/revoke` (tenant API key): checked, then forwarded to identity.
- `GET /v1/secrets`, `PUT /v1/secrets/{kind}`, `DELETE /v1/secrets/{id}`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate` (tenant API key): forwarded to the vault with a token minted for **its** audience. The vault enforces `settings:manage`; the gateway validates kinds and UUIDs and forwards nothing else. No response carries a value.
- Tenant calls: asks `identity.authenticate` on **every** request (no cache, so revocation is immediate), then forwards with a 30-second EdDSA token carrying tenant, user, key and permissions.
- Operator calls: the bearer's SHA-256 is compared in constant time with the configured hash(es); forwards with an operator token.
- Builds a **new** request to the service: no client header, query string or API key is forwarded (tested with an exact header allowlist). Only the list route's `includeRevoked` is forwarded, and only in canonical form.
- Every unusable credential gets the same 401. 413 above 64 KiB (after authentication). 502 if the service cannot be reached or answers oddly, 504 if it is too slow. A service's 5xx, or its refusal of the gateway's own token, is never relayed.
- **Rate limits**: a token bucket per firm (default 600 a minute, burst 120) checked after authentication and before the body is read; the operator has its own bucket (60 a minute). Over budget is 429 with `Retry-After` in whole seconds. Buckets live in process, so they reset on restart and each replica has its own.
- **Metering**: one metadata-only JSON line on stdout per handled request (time, caller kind, tenant/user/key ids, route, service, tool, method, status, duration, bytes, whether it was rate limited). Never the body or the credential. Running totals per firm since start are on the loopback-only `GET /internal/usage`.
- **MCP routing** (`POST /mcp`, tenant API key): reads `MCP-Protocol-Version` (must be `2026-07-28`), `Mcp-Method` and `Mcp-Name`, and forwards a `tools/call` to the service that owns the tool in [`contracts/mcp/routes.json`](../../contracts/mcp/routes.json) — without parsing the body. The service receives a token minted for *its* audience and an allowlist of headers: `Content-Type`, the three MCP headers and `Origin`. A malformed header set is 400 (after authentication), an unknown tool is 404, a valid method it does not route or a tool whose service has no configured URL is 501.
- `/v1/authenticate` is internal and is a 404 from outside.

## Not yet (later)
The outward MCP aggregate (listing and `tools/list` across services), OIDC, traceparent propagation, idempotency keys, retries, a shared rate-limit store. Limits cannot protect identity from invalid-key floods because authentication comes first (ADR-0013, known gap).

## Contract
[`contracts/openapi/gateway.v1.yaml`](../../contracts/openapi/gateway.v1.yaml) (0.4.0). The internal `/internal/usage` endpoint is deliberately not in the tenant-facing contract.

## Configuration (environment)
`TIMELEX_GATEWAY_SIGNING_KEY`, `TIMELEX_GATEWAY_KEY_ID` (default `k1`), `TIMELEX_OPERATOR_KEY_SHA256`, `TIMELEX_SERVICE_URLS` (default `identity=http://127.0.0.1:8081`; `identity` is required, and `vault=http://127.0.0.1:8082` turns on the secrets routes), `TIMELEX_RATE_LIMIT_PER_MINUTE` (default 600), `TIMELEX_RATE_LIMIT_BURST` (default 120), `TIMELEX_GATEWAY_ADDR` (default `127.0.0.1:8088`), `TIMELEX_GATEWAY_INTERNAL_ADDR` (default `127.0.0.1:8089`; a non-loopback address logs a warning). It refuses to start without the first and third, and on a malformed value. Never commit values.

## Non-goals
- No business rules, no data storage, no model calls.

## Run
```bash
cargo run -p timelex-gateway -- keygen   # development key pair
cargo run -p timelex-gateway             # see deploy/README.md for the full local set-up
cargo test -p timelex-gateway            # the end-to-end tests need TIMELEX_TEST_DATABASE_URL
```

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
