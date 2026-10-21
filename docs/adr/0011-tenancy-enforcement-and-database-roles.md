# ADR-0011: Tenancy enforcement and database roles

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Implements [ADR-0001](0001-service-boundaries-and-contract-first.md) (schema and role per service) and [ADR-0007](0007-tenancy-and-data-residency.md) (row-level security). Tasks P1-01, P1-02, P1-03. SQL: [`deploy/postgres/bootstrap/`](../../deploy/postgres/bootstrap/) and [`services/identity/migrations/`](../../services/identity/migrations/). Tests: [`crates/timelex-db/tests/`](../../crates/timelex-db/tests/).

## Context
Law-firm data must not cross between firms, and one service must not read another's tables. ADR-0001 and ADR-0007 chose a shared Postgres cluster with a schema and role per service and row-level security by tenant. This ADR fixes the mechanism and its limits. Kone reviewed the SQL before it was applied and approved the choices below on 20 Oct 2026.

## Decision
1. **One schema and one login role per service**, created only by `platform.provision_service(name)`. The role (`svc_<name>`) is `NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS`, may use only its own schema (plus `platform`, to evaluate the tenancy function), cannot create objects, and has `search_path` set to its schema. A separate `timelex_migrator` owns the schemas and runs each service's migrations. `PUBLIC` has no rights on the database, `public` or any service schema.
2. **Roles carry no passwords in the repository.** They are created `NOLOGIN`; deployment sets login and credentials. Tests use a throwaway cluster that trusts local connections.
3. **Tenancy is enforced by Postgres.** A transaction declares its tenant with `set_config('timelex.tenant_id', uuid, true)` (transaction-local). Every tenant-owned table gets one policy, `tenant_isolation`, from `platform.apply_tenant_rls()`: `tenant_id = current_tenant()` for reads and writes, with `ENABLE` and `FORCE` row-level security. No tenant set means no rows; a malformed value is an error. Both fail closed.
4. **The only way to query is through a tenant.** `timelex_db::TenantPool::begin(tenant)` returns a `TenantTx`; services do not hold a raw pool.
5. **A CI guard**: the view `platform.tables_without_tenant_rls` must be empty, so a new table cannot ship without the standard policy.
6. **Identity data model.** Tables `tenants`, `users`, `user_roles`, `api_keys`. Composite keys `(tenant_id, id)` and composite foreign keys make a cross-tenant reference impossible. Firms and people are suspended or disabled, never deleted; the service role has no `DELETE` except on `user_roles`. A key's hash, scopes, owner and expiry cannot change (column-level `UPDATE` grants); rotation is a new key plus a revocation.
7. **API-key tokens embed the tenant**: `tlx_<tenant id, 32 hex>.<43-character secret>`. The prefix lets the service set the tenant before authenticating; the stored SHA-256 hash must match *inside* that tenant. Only the hash is stored.
8. **No passwords or local sign-in in Phase 1.** People will sign in through their firm's Microsoft or Google account (later); only machines use API keys. A firm is created by a platform operator in one transaction with its first admin.
9. **Rules live in Rust** (`timelex-identity`); the SQL constraints are a second line of defence, and a property test checks that every value the Rust rules accept is accepted by the database (slugs must agree exactly).

## Alternatives considered
- *A second, narrow role allowed to read `api_keys` across tenants for lookup.* Works, but adds a role and a standing cross-tenant exception to audit. Rejected in favour of the tenant-in-token design.
- *`SECURITY DEFINER` lookup function.* Hides the same exception inside a function; same objection.
- *Application-level `WHERE tenant_id = ...` only.* One missed clause leaks a firm. Rejected (ADR-0007).
- *A database or schema per tenant from the start.* Stronger, heavier. Remains the upgrade path (ADR-0007 point 4); tenant ids stay in contracts.
- *Passwords for people in Phase 1.* Adds credential storage and reset flows we do not want to own; SSO is the target.

## Consequences
- **Limit, stated plainly:** row-level security protects against bugs (a missing `WHERE`, a confused deputy) and cross-tenant reads by a correctly behaving service. A service that is fully compromised holds a role that can set any tenant id itself. The defence against that is the gateway's signed tenant claim and service identity (P1-04, threat model T3 and T7), not this layer.
- The tenant id is visible inside API keys. It is not secret, but it tells a holder which firm a leaked key belongs to.
- Role changes keep no history until the outbox (P1-06) and audit log (P1-14) exist.
- A suspended firm is enforced in Rust at authentication, not by the database.
- Every new service needs a provisioning line and a migration that calls `apply_tenant_rls`; the CI guard enforces the second.
- Tests need a throwaway Postgres; they fail loudly without one (`TIMELEX_SKIP_DB_TESTS=1` to skip on purpose).
- A mutation check was run once by hand: replacing the policy with `USING (true)` made 7 of 11 tenancy tests fail. Repeat it if the policy code changes.
- Revisit at the end of Phase 2 with real per-request cost, and before any hosted tenant (P8-05).
