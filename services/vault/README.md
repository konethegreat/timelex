# vault

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ v0 done: contract 0.1.0 plus the internal lease contract 0.1.0, domain crate (27 unit tests), sealed schema, store (16 database tests, including the queued `secret.*` events and leases), HTTP layer and provider check (5 unit, 17 HTTP tests), outbox relay in the binary |
| **Language** | Rust |
| **Phase** | 1 |
| **Owns data** | `vault` schema: encrypted secrets, wrapped data keys, audit chain |

## Purpose
Custody of each tenant's bring-your-own keys and OAuth tokens ([ADR-0004](../../docs/adr/0004-byo-keys-and-secrets-vault.md)). The only service that can decrypt.

## Responsibilities
- Envelope encryption: a fresh AES-256-GCM data key per secret, wrapped by the vault's key-encryption key
- Write-only API: create, list metadata, test, rotate and revoke; a value is never returned to a user
- One internal lease: a trusted service may receive a value once, for a declared purpose and 60 seconds ([ADR-0020](../../docs/adr/0020-workload-identity-and-leases.md))
- Typed secrets with allowed-host lists; tamper-evident access log (hash chain, per firm)

## What exists today
- **[`crates/timelex-vault`](../../crates/timelex-vault)** (no I/O): the four secret kinds and their provider hosts; value, label and host validation; a zeroising `SecretValue`; envelope encryption with the tenant, secret id and kind bound into both seals; the per-firm hash chain. Entropy and the clock are passed in, so every rule runs with fixed inputs in a test.
- **Schema** ([`migrations/0001_vault.sql`](migrations/0001_vault.sql)): `secrets` (all sealed columns NULL together, a partial unique index for one live secret per kind, values impossible to restore after revocation), `secret_audit` (append-only; no UPDATE or DELETE is granted), `chain_heads` (the row lock that keeps a chain from forking). All three tables carry the standard tenant policy.
- **Store** (`src/store.rs`): put (create or replace), list metadata, rotate, revoke, open-for-test, record-test and lease, each in one tenant transaction with its audit entry — and, for every change, its `secret.*` event. `settings:manage` is required for every administrative operation; a lease is scoped by its purpose instead.
- **Lease** (`VaultStore::lease`): locks the firm's one live secret of the kind `FOR UPDATE`, opens it, stamps `last_used_at`, appends a `used` audit entry (`lease <service> <purpose>`, actor the initiating user) and returns the value with `expiresAt` 60 seconds out. It emits no event: an access is not a change.
- **Events** ([`timelex-outbox`](../../crates/timelex-outbox)): the event is appended on the transaction's connection, so a rollback removes state, audit and event together. Created, updated, tested, rotated and revoked each queue one event carrying metadata only, never a value. The binary runs the relay against `platform.outbox` and writes one JSON line per published event ([ADR-0014](../../docs/adr/0014-transactional-outbox.md)).
- **HTTP layer** (`src/http.rs`): the five contract routes plus `/health`, internal `principal` tokens signed by the gateway, RFC 9457 problem details, a 64 KiB body limit, and no response or error body that can carry a value — except the internal lease, which is the one response with a value and is `no-store`.
- **Workload identity** ([`timelex-auth`](../../crates/timelex-auth/src/workload.rs)): the lease route accepts a token the calling service signs itself (`iss` `timelex-<service>`, kind `workload`, claims `svc`, `tid`, `sub`, `purpose`), checked against `TIMELEX_WORKLOAD_PUBLIC_KEYS`. No key configured means every lease is 401.
- **Provider check** (`src/validator.rs`): a `ProviderValidator` trait; `anthropic_api_key` makes a read-only `GET /v1/models` with the key in a header and reads only the status code. Other kinds pass on shape. A rejected key is 422 and is recorded; a provider that cannot be reached is 502 and is **not** recorded, so an outage cannot look like a bad key.
- **Binary** (`src/main.rs`): `serve` and `migrate`, with the master key decoded from `TIMELEX_VAULT_MASTER_KEY` and never printed.

## Contract (0.1.0)
[`contracts/openapi/vault.v1.yaml`](../../contracts/openapi/vault.v1.yaml) and, for the lease,
[`contracts/openapi/vault.internal.v1.yaml`](../../contracts/openapi/vault.internal.v1.yaml).
- **REST:** `PUT /v1/secrets/{kind}` (create or replace), `GET /v1/secrets`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}` (revoke).
- **Internal:** `POST /internal/v1/secrets/{kind}/lease`, a workload token only, purpose-checked before lookup; `model_run` is the only purpose and it grants `anthropic_api_key`.
- **Callers:** administrative routes take internal tokens of kind `principal` with `settings:manage`; the tenant comes from the token. The lease takes a workload token from a service whose key is configured. Only the gateway and trusted services reach this service.
- **Events published:** `secret.created`, `secret.updated`, `secret.tested`, `secret.rotated`, `secret.revoked` (metadata only, never a value). A lease publishes nothing. **Consumed:** `tenant.created`.

## Not in 0.1.0
- **MCP tools** (`list_secrets`, `test_secret`, `revoke_secret`) are in the catalogue but served through the adapter in P1-15, not hand-rolled here.
- **Workload Identity Federation** (ADR-0004 amendment) and KMS-held keys wait on T0-L05; 0.1.0 uses a 32-byte master key from the environment behind a `KeyWrapper` trait, so a KMS adapter is a drop-in. The workload tokens of ADR-0020 are TimeLex's own service identity, not a federation with a provider.

## Security notes
- Values never appear in logs, traces, error bodies or user-facing API responses; those responses carry at most the last four characters. The one exception is the internal lease response, which is `no-store` and carries the value for its caller's use.
- Plaintext exists only inside the vault process, in a zeroising wrapper whose `Debug`/`Display` are redacted, and on the lease wire hop that must stay private to services.
- Testing a secret is not use: it sets `lastTestedAt`, never `lastUsedAt`. A lease sets `lastUsedAt` and audits a `used` entry with the service and purpose.
- Revoking destroys the wrapped data key, so even a database copy cannot decrypt the value afterwards.
- A lease expires 60 seconds after issue by contract; the vault keeps no copy to recall, so the caller must drop the value at expiry or when the run ends, whichever comes first.

## Non-goals
- Never returns plaintext to a user or an admin; only a trusted service under a purpose-scoped lease receives it
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
| `TIMELEX_WORKLOAD_PUBLIC_KEYS` | `service:kid=base64[,...]` keys of the services allowed to lease, e.g. `mcp-host:mh1=...`. Absent means every lease is refused (fail closed) |
| `TIMELEX_VAULT_MASTER_KEY` | base64 of exactly 32 bytes: the key-encryption key. The only thing that can unwrap a stored data key, so it belongs in a secret store, never in a file or an image |
| `TIMELEX_VAULT_ADDR` | listen address, default `127.0.0.1:8082`; keep it private to the gateway |
| `TIMELEX_OUTBOX_RELAY` | `0` disables the relay; the default is enabled |
| `TIMELEX_OUTBOX_INTERVAL_MS` | relay poll interval, default 500 |

See [TASKS.md](../../docs/TASKS.md) for the remaining vault work (MCP tools in P1-15; a KMS key wrapper later).

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
