# gateway

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ walking skeleton (`/health`, `/v1/meta`; 3 tests) |
| **Language** | Rust (axum) |
| **Phase** | 1 |
| **Owns data** | none (config only) |

## Purpose
Edge of the platform. One front door for REST and (outward) MCP: authentication, tenant resolution, rate limits, metering, routing and OpenAPI/MCP aggregation.

## Responsibilities
- Authenticate API keys and OIDC tokens; resolve the tenant (P1-03, P1-04)
- Route by `Mcp-Method` / `Mcp-Name` headers without parsing bodies (MCP 2026-07-28)
- Per-tenant rate limits and usage metering
- Aggregate each service's OpenAPI and MCP catalogue behind one authz layer

## Contract
- **REST:** `GET /health`, `GET /v1/meta` (built). Later: proxy of every service under `/v1/<service>/…`.
- **MCP tools:** Aggregated `/mcp` (Phase 1, P1-08).
- **Events published:** none
- **Events consumed:** none

## Non-goals
- No business rules
- No data storage
- No model calls

## Run
```bash
cargo run -p timelex-gateway   # http://127.0.0.1:8088/health   (TIMELEX_GATEWAY_ADDR to change)
cargo test -p timelex-gateway
```

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
