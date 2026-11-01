# contracts/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

The contracts are the product's source of truth ([ADR-0001](../docs/adr/0001-service-boundaries-and-contract-first.md)). Code follows them; CI fails if code and contract drift.

| Path | What | Checked by |
|---|---|---|
| `events/*.schema.json` | JSON Schema 2020-12 for event payloads; how they change is in [`events/README.md`](events/README.md) | `cargo test -p timelex-types` validates the current example and every frozen released example |
| `examples/*.json` | Golden examples used by tests and docs; `*.v<major>.<minor>.json` are frozen when a minor is released | Same tests; `packages/sdk` test |
| `openapi/*.yaml` | REST contracts, OpenAPI 3.1 | Gateway tests today; generated SDK and contract tests (P1-09) |
| `mcp/tools.md` | MCP tool catalogue (draft) | Checked against Rust types from P1-08 |
| `mcp/routes.json` | Tool-to-service routing for the gateway (ADR-0013) | `services/gateway/tests/contracts.rs` checks it against `tools.md` |
| `internal-auth.md` | How services trust the gateway (internal token), draft | ADR-0012; `crates/timelex-auth` (20 tests), gateway and identity tests |

Rules: [event contracts](events/README.md) have their own versioning rules: optional additions only within a major, and readers ignore fields they do not know. For every contract, a breaking change needs a new major (`/v2`, `schemaVersion` 2.x) and a deprecation window. Never put real client data in examples; use the reserved `.test` and `.example` domains.
