# new-service

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Generates `services/<name>` with the platform's shape already in place, then wires it into the
workspace and the Postgres bootstrap. P1-11; design and reasons in [ADR-0017](../../docs/adr/0017-new-service-generator.md).

## Use

From the repository root:

```
cargo run -p timelex-new-service -- <name>
```

`<name>` is 2 to 31 characters: a lowercase letter, then lowercase letters, digits or underscores.
That is the same rule `platform.provision_service` enforces, because the name becomes the schema and
the service role `svc_<name>`. It also becomes the directory name and the crate
`timelex-<name>-service`.

```
cargo run -p timelex-new-service -- matters
```

The tool refuses a name whose `services/<name>` directory already exists. It never merges with
existing work.

## What it creates

| File | Contains |
|---|---|
| `Cargo.toml` | the workspace settings and the same dependencies the existing services use |
| `src/lib.rs` | the embedded `MIGRATIONS` and the public surface |
| `src/main.rs` | `serve` (default) and `migrate`, configuration from the environment |
| `src/http.rs` | router, `/health`, RFC 9457 problem bodies, the gateway-token extractor, two example routes |
| `src/store.rs` | tenant-scoped queries through `timelex_db::TenantTx` |
| `src/error.rs` | store errors before the HTTP layer maps them |
| `migrations/0001_items.sql` | one tenant-owned table with the standard RLS policy and grants |
| `tests/http.rs` | database-backed tests: health, authentication, tenant isolation, input errors |
| `README.md` | the service's own page, with the commands and environment variables |

The `items` resource is an example. Replace it with the operations of the service's contract; the
tests are the cheap proof that the replacement kept tenancy intact.

## What it patches

Only two existing files:

1. the `members` list in the root `Cargo.toml`, so the workspace (and therefore `cargo test
   --workspace` and CI) covers the new crate;
2. `deploy/postgres/bootstrap/004_provision_services.sql`, so the role, schema and grants exist when
   the bootstrap runs.

Both edits are idempotent. Nothing else in the repository is touched: contracts, gateway routes and
compose services are decisions that need a human.

## What comes for free

- **Authentication:** every route except `/health` demands an internal token signed by the gateway
  and accepts only a tenant principal ([ADR-0012](../../docs/adr/0012-internal-service-authentication.md)).
- **Tenancy:** every query runs inside a tenant transaction under row-level security
  ([ADR-0011](../../docs/adr/0011-tenancy-enforcement-and-database-roles.md)). The generated test
  proves one firm cannot see another's rows.
- **Health:** `GET /health` pings the database and returns the same body as every other service.
- **Telemetry:** `init` and `instrument` join the caller's trace, emit the request metrics and
  redact logs ([ADR-0016](../../docs/adr/0016-telemetry-kit.md)).
- **CI:** being a workspace member is enough; the Rust job formats, lints and tests it. The job
  also generates a scratch `example` service and checks it, so the templates cannot rot.

## After generating

1. Add the service's contract under `contracts/openapi/` and replace the example operations.
2. Route the new operations from the gateway.
3. Add the service to `deploy/docker-compose.yml` when it should run in the development stack.
4. Run the tests with a throwaway Postgres:
   ```
   TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-<name>-service
   ```

## Tests

```
cargo test -p timelex-new-service
```

The tests generate into a temporary workspace, so they never touch the repository.
