// Written by Kone Tshivhinda & Claude
//! Test support: a throwaway Postgres database per test, set up exactly as production is.
//! Compiled only with the `testing` feature, which crates enable as a dev-dependency.
//!
//! Needs `TIMELEX_TEST_DATABASE_URL`, a superuser URL for a **throwaway test cluster that trusts
//! local connections** (no passwords exist anywhere in these tests), for example
//!
//! ```text
//! docker run -d --rm -e POSTGRES_HOST_AUTH_METHOD=trust -p 127.0.0.1:55432:5432 postgres:17
//! TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test --workspace
//! ```
//!
//! If the variable is missing the tests FAIL, because a silently skipped isolation test is worse than
//! no test. Set `TIMELEX_SKIP_DB_TESTS=1` to skip them on purpose.

use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

use crate::{bootstrap, TenantPool};

/// Postgres advisory-lock key serialising setup. Roles belong to the whole cluster, not to one
/// database, so two tests creating or altering them at once could collide.
const SETUP_LOCK: i64 = 7_001_001;

/// A service whose schema a test needs: its name (`identity`) and its embedded migrations.
pub struct ServiceUnderTest {
    /// Service name, as given to `platform.provision_service`.
    pub service: &'static str,
    /// The service's migrations, usually `sqlx::migrate!("./migrations")`.
    pub migrations: &'static Migrator,
}

/// One test's own database.
pub struct TestDb {
    /// Database name (random, `tlx_test_...`).
    pub name: String,
    base: PgConnectOptions,
    service: &'static str,
    /// Superuser pool on this test's own database.
    pub admin: PgPool,
}

/// Create a fresh database, run the bootstrap and the service's migrations. Returns `None` only
/// when skipping was requested explicitly.
pub async fn setup(spec: &ServiceUnderTest) -> Option<TestDb> {
    let url = match std::env::var("TIMELEX_TEST_DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            if std::env::var("TIMELEX_SKIP_DB_TESTS").as_deref() == Ok("1") {
                eprintln!("SKIPPING database tests (TIMELEX_SKIP_DB_TESTS=1)");
                return None;
            }
            panic!(
                "TIMELEX_TEST_DATABASE_URL is not set. These tests need a throwaway Postgres \
                 (see crates/timelex-db/src/testing.rs). Set TIMELEX_SKIP_DB_TESTS=1 to skip."
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
    // The bootstrap provisions `identity`; any other service under test is provisioned here
    // (idempotent). Roles are created NOLOGIN; a test cluster that trusts local connections lets them
    // log in once they are allowed to. (Production sets real credentials at deploy time.)
    sqlx::query("CALL platform.provision_service($1)")
        .bind(spec.service)
        .execute(&admin)
        .await
        .unwrap();
    for role in ["timelex_migrator".to_string(), format!("svc_{}", spec.service)] {
        sqlx::raw_sql(&format!("ALTER ROLE {role} LOGIN"))
            .execute(&admin)
            .await
            .unwrap();
    }

    // The service's own migrations, run as the migrator with search_path = the service schema.
    let migrator = connect_as(&base, &name, "timelex_migrator", Some(spec.service)).await;
    spec.migrations.run(&migrator).await.expect("migration failed");
    migrator.close().await;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SETUP_LOCK)
        .execute(&mut lock)
        .await
        .unwrap();
    Some(TestDb {
        name,
        base,
        service: spec.service,
        admin,
    })
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

    /// The service's own pool: the raw pool (to test what happens *without* a tenant) and the
    /// tenant-only wrapper services really use.
    pub async fn service_pool(&self) -> (PgPool, TenantPool) {
        let raw = self.as_role(&format!("svc_{}", self.service)).await;
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

    /// Drop this test's database.
    pub async fn cleanup(self) {
        let TestDb {
            name, base, admin, ..
        } = self;
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
