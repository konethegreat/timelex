# identity

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ library, HTTP layer and binary (16 store tests, 11 HTTP tests); reached only through the gateway |
| **Language** | Rust |
| **Phase** | 1 |
| **Owns data** | `identity` schema: tenants, users, roles, API keys (SSO settings later) |

## Purpose
Who is allowed to do what, in which tenant.

## What exists (P1-03)
Four operations, defined first in [`contracts/openapi/identity.v1.yaml`](../../contracts/openapi/identity.v1.yaml):

| Operation | Does | Who |
|---|---|---|
| `createTenant` | Creates a firm, its first admin and that admin's first key, in one transaction | platform operator (gateway decides) |
| `issueApiKey` | Issues a scoped key for an active user; scopes cannot exceed the owner's roles | `keys:manage` |
| `revokeApiKey` | Revokes a key at once | `keys:manage` |
| `authenticate` | Resolves a token to a principal; every failure looks the same to the caller | gateway only |

Rules live in [`crates/timelex-identity`](../../crates/timelex-identity); the store here loads facts, calls them and writes. Design and limits: [ADR-0011](../../docs/adr/0011-tenancy-enforcement-and-database-roles.md).

## Not yet
Create/invite/disable users, role changes, SSO, listing keys, idempotency keys (ADR-0010; see the contract's note), audit events (outbox P1-06, audit log P1-14).

## Contract
- **REST:** `/v1/tenants`, `/v1/api-keys`, `/v1/api-keys/{keyId}/revoke`, `/v1/authenticate`
- **MCP tools:** `list_users` (R), `create_api_key` (W, admin only): catalogue only, not implemented
- **Events published:** `tenant.created`, `user.invited`, `apikey.revoked`: not yet emitted

## Non-goals
- Does not store model or connector secrets (that is `vault`)
- No billing or matter data

## Run the tests
Need a throwaway Postgres: see [deploy/postgres/README.md](../../deploy/postgres/README.md).
```
TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-identity-service
```

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).

## HTTP layer (P1-04 part a)
`timelex-identity-service` serves the contract on `127.0.0.1:8081` by default and **listens privately**: it accepts only internal tokens signed by the gateway ([ADR-0012](../../docs/adr/0012-internal-service-authentication.md)), never a tenant API key, and each route accepts one kind of caller (`operator`, `principal` or `service`). Wrong kind is a 403, anything unverifiable is the same 401. Request bodies are limited to 64 KiB and error bodies never echo the input. Commands: `timelex-identity-service` (serve) and `timelex-identity-service migrate`. Environment: `TIMELEX_IDENTITY_DATABASE_URL`, `TIMELEX_MIGRATOR_DATABASE_URL`, `TIMELEX_GATEWAY_PUBLIC_KEYS`, `TIMELEX_IDENTITY_ADDR`.
