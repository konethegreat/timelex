// Written by Kone Tshivhinda & Claude
//! Test harness: a throwaway Postgres database per test, set up exactly as production is.
//!
//! Needs `TIMELEX_TEST_DATABASE_URL`, a superuser URL for a **throwaway test cluster that trusts
//! local connections** (no passwords exist anywhere in these tests), for example
//!
//! ```text
//! docker run -d --rm -e POSTGRES_HOST_AUTH_METHOD=trust -p 127.0.0.1:55432:5432 postgres:17
//! TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-db
//! ```
//!
//! If the variable is missing the tests FAIL, because a silently skipped isolation test is worse than
//! no test. Set `TIMELEX_SKIP_DB_TESTS=1` to skip them on purpose.

#![allow(dead_code)] // each test file uses a different part of the harness

use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool};
use timelex_db::{bootstrap, TenantPool};
use timelex_identity::{ApiKeyToken, TenantId};
use uuid::Uuid;

/// Postgres advisory-lock key serialising setup. Roles belong to the whole cluster, not to one
/// database, so two tests creating or altering them at once could collide.
const SETUP_LOCK: i64 = 7_001_001;

pub struct TestDb {
    pub name: String,
    base: PgConnectOptions,
    /// Superuser pool on this test's own database.
    pub admin: PgPool,
}

/// Create a fresh database and set it up. Returns `None` only when skipping was requested.
pub async fn setup() -> Option<TestDb> {
    let url = match std::env::var("TIMELEX_TEST_DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            if std::env::var("TIMELEX_SKIP_DB_TESTS").as_deref() == Ok("1") {
                eprintln!("SKIPPING database tests (TIMELEX_SKIP_DB_TESTS=1)");
                return None;
            }
            panic!(
                "TIMELEX_TEST_DATABASE_URL is not set. These tests need a throwaway Postgres \
                 (see crates/timelex-db/tests/common/mod.rs). Set TIMELEX_SKIP_DB_TESTS=1 to skip."
            );
        }
    };
    let base = PgConnectOptions::from_str(&url).expect("TIMELEX_TEST_DATABASE_URL is not a valid URL");
    let name = format!("tlx_test_{}", Uuid::new_v4().simple());

    // One connection holds the lock for the whole setup.
    let mut lock = PgConnection::connect_with(&base)
        .await
        .expect("connect to the test cluster");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(SETUP_LOCK)
        .execute(&mut lock)
        .await
        .unwrap();
    sqlx::raw_sql(&format!("CREATE DATABASE {name}"))
        .execute(&mut lock)
        .await
        .unwrap();

    let admin = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(base.clone().database(&name))
        .await
        .unwrap();
    bootstrap(&admin).await.expect("bootstrap SQL failed");
    // Roles are created NOLOGIN by the bootstrap; a test cluster that trusts local connections lets
    // them log in once they are allowed to. (Production sets real credentials at deploy time.)
    for role in ["timelex_migrator", "svc_identity"] {
        sqlx::raw_sql(&format!("ALTER ROLE {role} LOGIN"))
            .execute(&admin)
            .await
            .unwrap();
    }

    // The identity service's own migrations, run as the migrator with search_path = identity.
    let migrator = connect_as(&base, &name, "timelex_migrator", Some("identity")).await;
    sqlx::migrate!("../../services/identity/migrations")
        .run(&migrator)
        .await
        .expect("identity migration failed");
    migrator.close().await;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SETUP_LOCK)
        .execute(&mut lock)
        .await
        .unwrap();
    Some(TestDb { name, base, admin })
}

async fn connect_as(base: &PgConnectOptions, db: &str, role: &str, search_path: Option<&str>) -> PgPool {
    let mut opts = base.clone().database(db).username(role);
    if let Some(sp) = search_path {
        opts = opts.options([("search_path", sp)]);
    }
    PgPoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap()
}

impl TestDb {
    /// A pool that logs in as a real login role (not `SET ROLE` from a superuser), so row-level
    /// security, `ALTER ROLE ... SET search_path` and privileges apply exactly as in production.
    pub async fn as_role(&self, role: &str) -> PgPool {
        connect_as(&self.base, &self.name, role, None).await
    }

    /// Connect options for a login role on this test database.
    pub fn options_for(&self, role: &str) -> PgConnectOptions {
        self.base.clone().database(&self.name).username(role)
    }

    /// The identity service's pool, wrapped so it can only be used through a tenant.
    pub async fn identity_pool(&self) -> (PgPool, TenantPool) {
        let raw = self.as_role("svc_identity").await;
        (raw.clone(), TenantPool::from_pool(raw))
    }

    /// Provision another service (as the admin) and let its role log in.
    pub async fn provision(&self, service: &str) {
        sqlx::query("CALL platform.provision_service($1)")
            .bind(service)
            .execute(&self.admin)
            .await
            .unwrap();
        sqlx::raw_sql(&format!("ALTER ROLE svc_{service} LOGIN"))
            .execute(&self.admin)
            .await
            .unwrap();
    }

    pub async fn cleanup(self) {
        let TestDb { name, base, admin } = self;
        admin.close().await;
        let mut c = PgConnection::connect_with(&base).await.unwrap();
        sqlx::raw_sql(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
            .execute(&mut c)
            .await
            .unwrap();
    }
}

/// The Postgres error code (for example `42501` permission denied), or `""` if not a database error.
pub fn code(err: &sqlx::Error) -> String {
    err.as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string())
        .unwrap_or_default()
}

/// Insert a firm with `users` users, each with one role and one API key, acting as that firm.
/// Returns the tenant id.
pub async fn seed_tenant(pool: &TenantPool, slug: &str, users: usize) -> TenantId {
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let mut tx = pool.begin(tenant).await.unwrap();
    sqlx::query("INSERT INTO identity.tenants (id, slug, display_name) VALUES ($1, $2, $3)")
        .bind(tenant.as_uuid())
        .bind(slug)
        .bind(format!("Firm {slug}"))
        .execute(tx.conn())
        .await
        .unwrap();
    let mut first: Option<Uuid> = None;
    for i in 0..users {
        let user = Uuid::new_v4();
        let first_id = *first.get_or_insert(user);
        sqlx::query("INSERT INTO identity.users (tenant_id, id, email, display_name, status) VALUES ($1, $2, $3, $4, 'active')")
            .bind(tenant.as_uuid())
            .bind(user)
            .bind(format!("user{i}@{slug}.example"))
            .bind(format!("User {i}"))
            .execute(tx.conn())
            .await
            .unwrap();
        sqlx::query("INSERT INTO identity.user_roles (tenant_id, user_id, role, granted_by) VALUES ($1, $2, 'fee_earner', $3)")
            .bind(tenant.as_uuid())
            .bind(user)
            .bind(first_id)
            .execute(tx.conn())
            .await
            .unwrap();
        let mut secret = [0u8; 32];
        secret[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        secret[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        let token = ApiKeyToken::new(tenant, secret);
        sqlx::query(
            "INSERT INTO identity.api_keys (tenant_id, id, user_id, name, key_prefix, key_hash, scopes, created_by) \
             VALUES ($1, $2, $3, 'test key', $4, $5, ARRAY['time:read'], $6)",
        )
        .bind(tenant.as_uuid())
        .bind(Uuid::new_v4())
        .bind(user)
        .bind(token.display_prefix())
        .bind(token.hash().as_bytes().as_slice())
        .bind(first_id)
        .execute(tx.conn())
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    tenant
}
