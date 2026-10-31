# Postgres bootstrap

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_
Why it is built this way: [ADR-0011](../../docs/adr/0011-tenancy-enforcement-and-database-roles.md), [ADR-0007](../../docs/adr/0007-tenancy-and-data-residency.md), and for the queue [ADR-0014](../../docs/adr/0014-transactional-outbox.md).

One database, one **schema** and one **login role** per service. Two layers of SQL:

| Layer | Files | Run by | When |
|---|---|---|---|
| **Bootstrap** | `bootstrap/001…004_*.sql` | an administrator (can create roles) | once per database; safe to repeat |
| **Service migrations** | `services/<name>/migrations/*.sql` | `timelex_migrator` | on every release of that service |

## What the bootstrap creates
- `timelex_migrator`: owns the service schemas; not a superuser; cannot bypass row-level security.
- `platform` schema: `current_tenant()`, `apply_tenant_rls()`, `provision_service()`, the view `tables_without_tenant_rls`, and `platform.outbox`.
- `platform.outbox`: the one queue every service writes events to, in the same transaction as its state change. A relay claims undelivered rows and publishes them at least once; consumers dedupe on the envelope id. The `platform` schema has no tenant policy because the relay must read across firms and no runtime role bypasses row-level security.
- For each provisioned service (today: `identity`, `vault`): a role `svc_<name>` and a schema `<name>`. The role can use only its own schema. It may `INSERT` and `SELECT` the outbox and `UPDATE` only the delivery columns (`published_at`, `attempts`, `next_attempt_at`, `last_error`), so a producer cannot rewrite a payload or an already-published event.

Roles are created `NOLOGIN` and **no file in this repository contains a password**.

## Local development
```
docker compose -f deploy/docker-compose.yml up -d postgres
# as the superuser from the compose file, run the four bootstrap files in order, then:
#   ALTER ROLE timelex_migrator LOGIN PASSWORD '...';   ALTER ROLE svc_identity LOGIN PASSWORD '...';
#   ALTER ROLE svc_vault LOGIN PASSWORD '...';
# choose your own passwords and keep them in your untracked .env
```
`timelex_db::bootstrap(&admin_pool)` runs the four files for you from Rust.

## Tests
The tenancy and role-isolation tests in `crates/timelex-db` need a **throwaway** Postgres that trusts local connections (so no password exists):
```
docker run -d --rm --name tlx-test-pg -e POSTGRES_HOST_AUTH_METHOD=trust -p 127.0.0.1:55432:5432 postgres:17
TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-db
```
Without the variable the tests **fail** on purpose; `TIMELEX_SKIP_DB_TESTS=1` skips them knowingly. CI runs them against a service container. Never point the variable at a database you care about: each test creates and drops its own database, and creates roles in the cluster.

## Adding a service
1. Add `CALL platform.provision_service('<name>');` to `bootstrap/004_provision_services.sql`.
2. Write the service's migration. Give each tenant-owned table a `tenant_id` column and call `CALL platform.apply_tenant_rls('<name>.<table>')`; grant the service role exactly the privileges it needs (column by column for anything that must not change).
3. The test `every_table_has_the_standard_policy` fails until step 2 is done.
4. Write events through `timelex_outbox::append` on the service's transaction; the provisioning procedure already grants the queue privileges described above.
