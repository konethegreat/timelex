// Written by Kone Tshivhinda & Claude
//! Harness for the identity-schema tests: the shared `timelex_db::testing` support plus a seeding
//! helper that knows the identity tables. Setup and the environment variables are described in
//! `crates/timelex-db/src/testing.rs`.

#![allow(dead_code)] // each test file uses a different part of the harness

use sqlx::migrate::Migrator;
use timelex_db::testing::ServiceUnderTest;
pub use timelex_db::testing::{code, TestDb};
use timelex_db::TenantPool;
use timelex_identity::{ApiKeyToken, TenantId};
use uuid::Uuid;

static IDENTITY_MIGRATIONS: Migrator = sqlx::migrate!("../../services/identity/migrations");

/// A fresh database with the bootstrap and the identity migration applied.
pub async fn setup() -> Option<TestDb> {
    timelex_db::testing::setup(&ServiceUnderTest {
        service: "identity",
        migrations: &IDENTITY_MIGRATIONS,
    })
    .await
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
