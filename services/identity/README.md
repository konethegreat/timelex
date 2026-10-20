# identity

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 1 |
| **Owns data** | `identity` schema: tenants, users, roles, API keys, SSO settings |

## Purpose
Who is allowed to do what, in which tenant.

## Responsibilities
- Create tenants and invite users; roles (admin, fee earner, billing, auditor, agent)
- Issue, scope, rotate and revoke API keys (hashed at rest)
- OIDC / SSO configuration for firms using Entra ID or Google
- Emit tenant and user lifecycle events

## Contract
- **REST:** `/v1/tenants`, `/v1/users`, `/v1/roles`, `/v1/api-keys`
- **MCP tools:** `list_users` (R), `create_api_key` (W, admin only)
- **Events published:** `tenant.created`, `user.invited`, `apikey.revoked`
- **Events consumed:** none

## Non-goals
- Does not store model or connector secrets (that is `vault`)
- No billing or matter data

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
