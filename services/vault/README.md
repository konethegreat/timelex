# vault

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 1 |
| **Owns data** | `vault` schema: encrypted secrets, wrapped data keys, audit chain |

## Purpose
Custody of each tenant's bring-your-own keys and OAuth tokens ([ADR-0004](../../docs/adr/0004-byo-keys-and-secrets-vault.md)). The only service that can decrypt.

## Responsibilities
- Envelope encryption: AES-256-GCM data key per secret, wrapped by a KMS-held key
- Write-only API: create, test, rotate, revoke; never read back
- Short-lived in-memory leases for `mcp-host` and `capture-hub`, scoped to tenant, purpose and provider host
- Typed secrets with allowed-host lists; tamper-evident access log

## Contract
- **REST:** `PUT /v1/secrets/{kind}`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}`; internal `POST /internal/v1/secrets/{id}/lease`
- **MCP tools:** `list_secrets` (R, metadata only), `test_secret` (R), `revoke_secret` (W)
- **Events published:** `secret.created`, `secret.rotated`, `secret.revoked`, `secret.used` (never contains values)
- **Events consumed:** `tenant.created`

## Non-goals
- Never returns plaintext over any public API
- Never logs, traces or prompts secrets
- No pooling of keys across tenants

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
