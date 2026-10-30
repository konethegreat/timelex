# vault

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🔄 contract 0.1.0 done; domain crate, sealed schema and store done (27 unit + 11 database tests); HTTP service and the Anthropic check next |
| **Language** | Rust |
| **Phase** | 1 |
| **Owns data** | `vault` schema: encrypted secrets, wrapped data keys, audit chain |

## Purpose
Custody of each tenant's bring-your-own keys and OAuth tokens ([ADR-0004](../../docs/adr/0004-byo-keys-and-secrets-vault.md)). The only service that can decrypt.

## Responsibilities
- Envelope encryption: a fresh AES-256-GCM data key per secret, wrapped by the vault's key-encryption key
- Write-only API: create, list metadata, test, rotate and revoke; a value is never returned
- Typed secrets with allowed-host lists; tamper-evident access log (hash chain, per firm)

## What exists today
- **[`crates/timelex-vault`](../../crates/timelex-vault)** (no I/O): the four secret kinds and their provider hosts; value, label and host validation; a zeroising `SecretValue`; envelope encryption with the tenant, secret id and kind bound into both seals; the per-firm hash chain. Entropy and the clock are passed in, so every rule runs with fixed inputs in a test.
- **Schema** ([`migrations/0001_vault.sql`](migrations/0001_vault.sql)): `secrets` (all sealed columns NULL together, a partial unique index for one live secret per kind, values impossible to restore after revocation), `secret_audit` (append-only; no UPDATE or DELETE is granted), `chain_heads` (the row lock that keeps a chain from forking). All three tables carry the standard tenant policy.
- **Store** (`src/store.rs`): put (create or replace), list metadata, rotate, revoke, open-for-test and record-test, each in one tenant transaction with its audit entry. `settings:manage` is required for every operation.
- **Not yet:** the HTTP layer, the provider validator and the binary.

## Contract (0.1.0)
[`contracts/openapi/vault.v1.yaml`](../../contracts/openapi/vault.v1.yaml).
- **REST:** `PUT /v1/secrets/{kind}` (create or replace), `GET /v1/secrets`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}` (revoke).
- **Callers:** internal tokens only, kind `principal`, permission `settings:manage`; the tenant comes from the token. Only the gateway reaches this service.
- **Events published:** `secret.created`, `secret.rotated`, `secret.revoked`, `secret.used` (never a value). **Consumed:** `tenant.created`.

## Not in 0.1.0
- **Internal lease** (`POST /internal/v1/secrets/{id}/lease`) waits for its first consumer, `mcp-host` (P1-13). It is the most dangerous surface, so it lands with tests and a caller that needs it.
- **MCP tools** (`list_secrets`, `test_secret`, `revoke_secret`) are in the catalogue but served through the adapter in P1-15, not hand-rolled here.
- **Workload Identity Federation** (ADR-0004 amendment) and KMS-held keys wait on T0-L05; 0.1.0 uses a 32-byte master key from the environment behind a `KeyWrapper` trait, so a KMS adapter is a drop-in.

## Security notes
- Values never appear in logs, traces, error bodies or API responses; the response carries at most the last four characters.
- Plaintext exists only inside the vault process, in a zeroising wrapper whose `Debug`/`Display` are redacted.
- Testing a secret is not use: it sets `lastTestedAt`, never `lastUsedAt`.
- Revoking destroys the wrapped data key, so even a database copy cannot decrypt the value afterwards.

## Non-goals
- Never returns plaintext over any API
- Never logs, traces or prompts secrets
- No pooling of keys across tenants

## Run
```bash
cargo test -p timelex-vault                 # rules and encryption, no database needed
cargo test -p timelex-vault-service         # the store needs TIMELEX_TEST_DATABASE_URL
```
See [TASKS.md](../../docs/TASKS.md) for the remaining P1-05 work.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
