// Written by Kone Tshivhinda & Claude
//! Database layer shared by TimeLex services.
//!
//! Three things live here:
//! 1. [`bootstrap`]: runs the admin-side SQL (roles, schemas, tenancy helpers) from
//!    `deploy/postgres/bootstrap/`.
//! 2. [`TenantPool`] and [`TenantTx`]: the only way a service talks to its tables. A transaction is
//!    always opened *for one tenant*, so a query without a tenant cannot be written by accident.
//! 3. Tests (in `tests/`) that prove the isolation promises of ADR-0007 against a real Postgres.
//!
//! This crate does I/O. Business rules do not live here: they live in domain crates such as
//! `timelex-identity` (ADR-0002).

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use timelex_identity::TenantId;

#[cfg(feature = "testing")]
pub mod testing;

/// The admin-side SQL files, in the order they must run. Embedded in the binary at build time so a
/// deployment cannot run them out of order or from the wrong version.
pub const BOOTSTRAP_FILES: [(&str, &str); 4] = [
    (
        "001_roles_and_schemas.sql",
        include_str!("../../../deploy/postgres/bootstrap/001_roles_and_schemas.sql"),
    ),
    (
        "002_tenancy.sql",
        include_str!("../../../deploy/postgres/bootstrap/002_tenancy.sql"),
    ),
    (
        "003_outbox.sql",
        include_str!("../../../deploy/postgres/bootstrap/003_outbox.sql"),
    ),
    (
        "004_provision_services.sql",
        include_str!("../../../deploy/postgres/bootstrap/004_provision_services.sql"),
    ),
];

/// Run the bootstrap SQL as an administrator (a role allowed to create roles). Safe to run again.
/// `raw_sql` sends each file as-is, so a file may hold many statements.
pub async fn bootstrap(admin: &PgPool) -> Result<(), sqlx::Error> {
    for (_name, sql) in BOOTSTRAP_FILES {
        sqlx::raw_sql(sql).execute(admin).await?;
    }
    Ok(())
}

/// A connection pool whose only door is [`TenantPool::begin`], which demands a tenant.
#[derive(Clone)]
pub struct TenantPool {
    pool: PgPool,
}

impl TenantPool {
    /// Connect with the given options (a service's own role) and a small pool.
    pub async fn connect(options: PgConnectOptions, max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    /// Wrap an existing pool (used by tests).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a transaction acting for `tenant`.
    ///
    /// Postgres row-level security then lets the transaction see and write only that tenant's rows.
    /// `set_config(..., true)` makes the setting last for this transaction only (it is cleared at
    /// commit or rollback), so a pooled connection cannot carry one request's tenant into the next.
    pub async fn begin(&self, tenant: TenantId) -> Result<TenantTx, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('timelex.tenant_id', $1, true)")
            .bind(tenant.as_uuid().to_string())
            .execute(&mut *tx)
            .await?;
        Ok(TenantTx { tx, tenant })
    }

    /// Check the database answers (`SELECT 1`). Touches no tenant data, so it needs no tenant.
    pub async fn ping(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await.map(|_| ())
    }

    /// Close the pool (waits for connections to be returned).
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// A transaction acting for one tenant. Dropping it without calling [`TenantTx::commit`] rolls back.
pub struct TenantTx {
    tx: Transaction<'static, Postgres>,
    tenant: TenantId,
}

impl TenantTx {
    /// The tenant this transaction acts for.
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// The connection to run queries on: `sqlx::query(..).execute(tx.conn())`.
    pub fn conn(&mut self) -> &mut PgConnection {
        &mut self.tx
    }

    /// Make the changes permanent.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }
}
