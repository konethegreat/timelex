# contracts/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

The contracts are the product's source of truth ([ADR-0001](../docs/adr/0001-service-boundaries-and-contract-first.md)). Code follows them; CI fails if code and contract drift.

| Path | What | Checked by |
|---|---|---|
| `events/*.schema.json` | JSON Schema 2020-12 for event payloads | `cargo test -p timelex-types` validates the golden example both ways |
| `examples/*.json` | Golden examples used by tests and docs | Same tests; `packages/sdk` test |
| `openapi/*.yaml` | REST contracts, OpenAPI 3.1 | Gateway tests today; generated SDK and contract tests (P1-09) |
| `mcp/tools.md` | MCP tool catalogue (draft) | Checked against Rust types from P1-08 |
| `internal-auth.md` | How services trust the gateway (internal token), draft | ADR-0012; tests arrive with P1-04 |

Rules: additive changes within a major version only; breaking changes need a new major (`/v2`, `schemaVersion` 2.x) and a deprecation window. Never put real client data in examples; use the reserved `.test` and `.example` domains.
