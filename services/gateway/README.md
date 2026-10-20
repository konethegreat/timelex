# gateway

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ P1-04 part a: authentication and routing to identity (13 stub-upstream tests, 5 end-to-end tests). Part b (rate limits, metering, MCP header routing) not started |
| **Language** | Rust (axum, reqwest) |
| **Phase** | 1 |
| **Owns data** | none (config only) |

## Purpose
Edge of the platform. One front door: it authenticates the caller, then routes to a service with a short-lived signed token. Design: [ADR-0012](../../docs/adr/0012-internal-service-authentication.md), [contracts/internal-auth.md](../../contracts/internal-auth.md).

## What it does today
- `GET /health`, `GET /v1/meta`: open.
- `POST /v1/tenants` (operator key), `POST /v1/api-keys`, `POST /v1/api-keys/{keyId}/revoke` (tenant API key): checked, then forwarded to identity.
- Tenant calls: asks `identity.authenticate` on **every** request (no cache, so revocation is immediate), then forwards with a 30-second EdDSA token carrying tenant, user, key and permissions.
- Operator calls: the bearer's SHA-256 is compared in constant time with the configured hash(es); forwards with an operator token.
- Builds a **new** request to the service: no client header, query string or API key is forwarded (tested with an exact header allowlist).
- Every unusable credential gets the same 401. 413 above 64 KiB (after authentication). 502 if the service cannot be reached or answers oddly, 504 if it is too slow. A service's 5xx, or its refusal of the gateway's own token, is never relayed.
- `/v1/authenticate` is internal and is a 404 from outside.

## Not yet (part b and later)
Per-tenant rate limits and metering (429), `Mcp-Method`/`Mcp-Name` routing and the outward `/mcp` aggregate, OIDC, traceparent propagation, idempotency keys, retries.

## Contract
[`contracts/openapi/gateway.v1.yaml`](../../contracts/openapi/gateway.v1.yaml) (0.2.0).

## Configuration (environment)
`TIMELEX_GATEWAY_SIGNING_KEY`, `TIMELEX_GATEWAY_KEY_ID` (default `k1`), `TIMELEX_OPERATOR_KEY_SHA256`, `TIMELEX_IDENTITY_URL` (default `http://127.0.0.1:8081`), `TIMELEX_GATEWAY_ADDR` (default `127.0.0.1:8088`). It refuses to start without the first and third. Never commit values.

## Non-goals
- No business rules, no data storage, no model calls.

## Run
```bash
cargo run -p timelex-gateway -- keygen   # development key pair
cargo run -p timelex-gateway             # see deploy/README.md for the full local set-up
cargo test -p timelex-gateway            # the end-to-end tests need TIMELEX_TEST_DATABASE_URL
```

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
