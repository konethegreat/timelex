# vault

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ v0 done: contract 0.1.0, domain crate (27 unit tests), sealed schema, store (12 database tests, including the queued `secret.*` events), HTTP layer and provider check (5 unit, 13 HTTP tests), outbox relay in the binary |
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
- **Store** (`src/store.rs`): put (create or replace), list metadata, rotate, revoke, open-for-test and record-test, each in one tenant transaction with its audit entry and its `secret.*` event. `settings:manage` is required for every operation.
- **Events** ([`timelex-outbox`](../../crates/timelex-outbox)): the event is appended on the transaction's connection, so a rollback removes state, audit and event together. Created, updated, tested, rotated and revoked each queue one event carrying metadata only, never a value. The binary runs the relay against `platform.outbox` and writes one JSON line per published event ([ADR-0014](../../docs/adr/0014-transactional-outbox.md)).
- **HTTP layer** (`src/http.rs`): the five contract routes plus `/health`, internal `principal` tokens signed by the gateway, RFC 9457 problem details, a 64 KiB body limit, and no response or error body that can carry a value.
- **Provider check** (`src/validator.rs`): a `ProviderValidator` trait; `anthropic_api_key` makes a read-only `GET /v1/models` with the key in a header and reads only the status code. Other kinds pass on shape. A rejected key is 422 and is recorded; a provider that cannot be reached is 502 and is **not** recorded, so an outage cannot look like a bad key.
- **Binary** (`src/main.rs`): `serve` and `migrate`, with the master key decoded from `TIMELEX_VAULT_MASTER_KEY` and never printed.

## Contract (0.1.0)
[`contracts/openapi/vault.v1.yaml`](../../contracts/openapi/vault.v1.yaml).
- **REST:** `PUT /v1/secrets/{kind}` (create or replace), `GET /v1/secrets`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}` (revoke).
- **Callers:** internal tokens only, kind `principal`, permission `settings:manage`; the tenant comes from the token. Only the gateway reaches this service.
- **Events published:** `secret.created`, `secret.updated`, `secret.tested`, `secret.rotated`, `secret.revoked` (metadata only, never a value). **Consumed:** `tenant.created`.

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
cargo test -p timelex-vault-service         # store and HTTP; needs TIMELEX_TEST_DATABASE_URL
```

The binary takes two commands: `timelex-vault-service migrate` (as `timelex_migrator`) and
`timelex-vault-service` / `serve`.

| Variable | Meaning |
|---|---|
| `TIMELEX_VAULT_DATABASE_URL` | connection URL for `svc_vault` (serve) |
| `TIMELEX_MIGRATOR_DATABASE_URL` | connection URL for `timelex_migrator` (migrate) |
| `TIMELEX_GATEWAY_PUBLIC_KEYS` | `kid=base64,...` public keys the gateway signs internal tokens with |
| `TIMELEX_VAULT_MASTER_KEY` | base64 of exactly 32 bytes: the key-encryption key. The only thing that can unwrap a stored data key, so it belongs in a secret store, never in a file or an image |
| `TIMELEX_VAULT_ADDR` | listen address, default `127.0.0.1:8082`; keep it private to the gateway |
| `TIMELEX_OUTBOX_RELAY` | `0` disables the relay; the default is enabled |
| `TIMELEX_OUTBOX_INTERVAL_MS` | relay poll interval, default 500 |

See [TASKS.md](../../docs/TASKS.md) for the remaining vault work (lease in P1-13, MCP tools in P1-15).

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
