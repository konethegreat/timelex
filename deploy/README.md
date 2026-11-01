# deploy/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

## The local development stack

`docker-compose.yml` runs the Phase 1 stack, all bound to loopback:

| Service | What it is | Where it listens |
|---|---|---|
| `postgres` | Postgres 17 | `127.0.0.1:55433` |
| `seaweedfs` | S3-compatible object storage (S3 gateway) | `127.0.0.1:9100` |
| `keys` | one-shot: generates the gateway key pair, the operator key and the vault master key | the `keys` volume |
| `bootstrap` | one-shot: re-applies the roles, schemas, tenancy helpers and dev logins on every start | |
| `identity-migrate`, `vault-migrate`, `capture-migrate` | one-shot migrations as `timelex_migrator` | |
| `identity` | identity service | `identity:8081`, container network only |
| `vault` | vault service plus its outbox relay | `vault:8082`, container network only |
| `capture` | capture service plus its outbox relay | `capture:8084`, container network only |
| `gateway` | public edge | `http://127.0.0.1:8088` |

```bash
docker compose -f deploy/docker-compose.yml up -d --build
docker compose -f deploy/docker-compose.yml ps          # wait for healthy
curl -s http://127.0.0.1:8088/health
```

`down` keeps the data volumes; `down -v` deletes them. The `bootstrap` container re-applies the Postgres bootstrap and the dev login grants on **every** `up`; the files are written to be safe to repeat, so a changed bootstrap (for example a newly provisioned service) reaches an existing volume without deleting data.

### Keys are generated, never committed

On first start the `keys` container runs `timelex-gateway keygen`, creates a random operator key and a 32-byte vault master key, and writes `/keys/env` and `/keys/operator-key` into the `keys` volume. Services source `/keys/env` from their entrypoint. The values are local-development secrets: do not reuse them, and do not expect them to survive `down -v`.

```bash
OPERATOR_KEY=$(docker compose -f deploy/docker-compose.yml exec -T gateway cat /keys/operator-key)
curl -sS -X POST http://127.0.0.1:8088/v1/tenants \
  -H "authorization: Bearer $OPERATOR_KEY" \
  -H 'content-type: application/json' \
  -d '{"slug":"example-attorneys","displayName":"Example Attorneys Inc","adminEmail":"partner@example-attorneys.example","adminDisplayName":"Pat Partner"}'
```

The response carries `initialApiKey.token` once; use it as the tenant's bearer token. Request counters are on the gateway's loopback-only listener inside the container:

```bash
docker compose -f deploy/docker-compose.yml exec gateway curl -s http://127.0.0.1:8089/internal/usage
```

### Trust authentication is for this stack only

The bootstrap creates service roles `NOLOGIN` and sets no passwords (see [postgres/README.md](postgres/README.md)). So that a local stack needs no committed credential, Postgres is started with `POSTGRES_HOST_AUTH_METHOD=trust` and the compose-only file `postgres/dev/005_dev_logins.sql` grants the roles `LOGIN`. The port is published to `127.0.0.1` only. Never expose this stack, and never run that file against a shared or hosted database: a real deployment sets passwords (or uses the platform's IAM authentication) and keeps the roles as the bootstrap leaves them.

### Object storage

MinIO's published images no longer resolve (checked 1 November 2026), so the dev stack uses SeaweedFS, which speaks S3 on port 8333 (published on `127.0.0.1:9100`). It runs without S3 credentials for now; configure them when the first service stores content (P2-02) and keep the port off any public interface.

The host ports are deliberately non-default (`55433` for Postgres, `9100` for S3): a developer machine often already runs a database on 5432 and another tool on 9000. Change the left-hand side in `docker-compose.yml` if those clash too; the container-side ports and the services' internal URLs do not change.

### Logs and traces

The three services write one JSON record per event (`TIMELEX_LOG_FORMAT=json`) at `RUST_LOG=info`, with sensitive field names and bearer tokens redacted before formatting. `docker compose logs gateway identity vault` shows them together.

Traces and metrics are exported only when `OTEL_EXPORTER_OTLP_ENDPOINT` is set. Point it at a collector running on the host (for example `http://host.docker.internal:4318` for OTLP/HTTP) and uncomment the line for the service you are debugging. Without it the services still share one trace id per request, so a log line in the gateway and the line for the same request in the vault carry the same id; nothing leaves the machine. See [ADR-0016](../docs/adr/0016-telemetry-kit.md).

### The Python worker

`workers/py-worker` is a library today: the redaction rules are exercised by `pytest`, and nothing consumes a queue yet. The compose stack therefore does not run it; its container belongs with the first queue consumer (P2-02 and P2-07).

## Building the image

The repository [`Dockerfile`](../Dockerfile) builds one image with the gateway, identity and vault binaries; compose selects one with `TIMELEX_BINARY`. It is a full release build with BuildKit cache mounts for the Cargo registry and `target`, so dependency compiles are reused between builds while a clean build takes several minutes. Use `cargo run` while iterating and let compose verify integration.

## Running services by hand

Useful when working on one service with `cargo run`. Start Postgres from the compose file (the bootstrap runs automatically), then run migrations once per release:

```bash
docker compose -f deploy/docker-compose.yml up -d postgres
cargo run -p timelex-gateway -- keygen           # prints a key pair: put the lines in your untracked environment
# Choose an operator key, store only its hash:
#   TIMELEX_OPERATOR_KEY_SHA256=$(printf %s "$YOUR_OPERATOR_KEY" | sha256sum | cut -d' ' -f1)
# Database URLs on the published host port (55433); the compose roles have LOGIN and trust auth,
# so there are no passwords:
#   TIMELEX_MIGRATOR_DATABASE_URL=postgres://timelex_migrator@127.0.0.1:55433/timelex
#   TIMELEX_IDENTITY_DATABASE_URL=postgres://svc_identity@127.0.0.1:55433/timelex
#   TIMELEX_VAULT_DATABASE_URL=postgres://svc_vault@127.0.0.1:55433/timelex
cargo run -p timelex-identity-service -- migrate
cargo run -p timelex-vault-service -- migrate
cargo run -p timelex-identity-service             # 127.0.0.1:8081
cargo run -p timelex-vault-service                # 127.0.0.1:8082, needs TIMELEX_VAULT_MASTER_KEY
cargo run -p timelex-gateway                      # 127.0.0.1:8088
```

Environment variables are documented in `services/gateway/src/config.rs`, `services/identity/src/main.rs` and `services/vault/src/main.rs`. The gateway reads service addresses from `TIMELEX_SERVICE_URLS` (default `identity=http://127.0.0.1:8081`), the firm budget from `TIMELEX_RATE_LIMIT_PER_MINUTE` and `TIMELEX_RATE_LIMIT_BURST`, and binds the usage listener on `TIMELEX_GATEWAY_INTERNAL_ADDR`; keep that address on loopback.
