// Written by Kone Tshivhinda & Claude
//! P1-02 acceptance: "a cross-tenant read fails in a property test", plus the guarantees around it.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::{code, seed_tenant, setup};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use timelex_db::TenantPool;
use timelex_identity::{Email, TenantId, TenantSlug};
use uuid::Uuid;

const PERMISSION_DENIED: &str = "42501";
const FK_VIOLATION: &str = "23503";
const CHECK_VIOLATION: &str = "23514";
const INVALID_TEXT: &str = "22P02";

async fn count(tx: &mut timelex_db::TenantTx, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT count(*) FROM identity.{table}"))
        .fetch_one(tx.conn())
        .await
        .unwrap()
}

#[tokio::test]
async fn every_table_has_the_standard_policy() {
    let Some(db) = setup().await else { return };
    let missing: Vec<(String, String)> =
        sqlx::query_as("SELECT schema_name::text, table_name::text FROM platform.tables_without_tenant_rls")
            .fetch_all(&db.admin)
            .await
            .unwrap();
    assert!(
        missing.is_empty(),
        "tables without tenant row-level security: {missing:?}"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn the_completeness_check_really_detects_an_unprotected_table() {
    let Some(db) = setup().await else { return };
    let migrator = db.as_role("timelex_migrator").await;
    sqlx::raw_sql("CREATE TABLE identity.oops (id int)")
        .execute(&migrator)
        .await
        .unwrap();
    let missing: Vec<(String, String)> =
        sqlx::query_as("SELECT schema_name::text, table_name::text FROM platform.tables_without_tenant_rls")
            .fetch_all(&db.admin)
            .await
            .unwrap();
    assert_eq!(missing, vec![("identity".to_string(), "oops".to_string())]);
    db.cleanup().await;
}

#[tokio::test]
async fn without_a_tenant_nothing_is_visible() {
    let Some(db) = setup().await else { return };
    let (raw, tenants) = db.service_pool().await;
    seed_tenant(&tenants, "firm-a", 2).await;
    for table in ["tenants", "users", "user_roles", "api_keys"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM identity.{table}"))
            .fetch_one(&raw)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} visible with no tenant set");
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_malformed_tenant_setting_fails_closed() {
    let Some(db) = setup().await else { return };
    let (raw, _) = db.service_pool().await;
    let mut tx = raw.begin().await.unwrap();
    sqlx::query("SELECT set_config('timelex.tenant_id', 'not-a-uuid', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let err = sqlx::query("SELECT count(*) FROM identity.users")
        .fetch_one(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(code(&err), INVALID_TEXT, "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn the_tenant_does_not_leak_to_the_next_transaction_on_the_same_connection() {
    let Some(db) = setup().await else { return };
    let (_, tenants) = db.service_pool().await;
    let a = seed_tenant(&tenants, "firm-a", 1).await;
    // A pool of one connection: the next transaction is guaranteed to reuse the same connection.
    let single = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(db_options(&db).await)
        .await
        .unwrap();
    let single_tenants = TenantPool::from_pool(single.clone());
    let mut tx = single_tenants.begin(a).await.unwrap();
    assert_eq!(count(&mut tx, "users").await, 1);
    tx.commit().await.unwrap();
    let leftover: Option<String> = sqlx::query_scalar("SELECT current_setting('timelex.tenant_id', true)")
        .fetch_one(&single)
        .await
        .unwrap();
    assert!(
        leftover.as_deref().unwrap_or("").is_empty(),
        "tenant setting survived the transaction: {leftover:?}"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.users")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(n, 0);
    db.cleanup().await;
}

/// Connect options for the identity role on the test database (for building a one-connection pool).
async fn db_options(db: &common::TestDb) -> sqlx::postgres::PgConnectOptions {
    db.options_for("svc_identity")
}

#[tokio::test]
async fn writing_a_row_for_another_tenant_is_rejected() {
    let Some(db) = setup().await else { return };
    let (_, tenants) = db.service_pool().await;
    let a = seed_tenant(&tenants, "firm-a", 1).await;
    let b = seed_tenant(&tenants, "firm-b", 1).await;
    // Acting as A, try to create a user inside B.
    let mut tx = tenants.begin(a).await.unwrap();
    let err = sqlx::query(
        "INSERT INTO identity.users (tenant_id, id, email, display_name) VALUES ($1, $2, 'x@firm-b.example', 'X')",
    )
    .bind(b.as_uuid())
    .bind(Uuid::new_v4())
    .execute(tx.conn())
    .await
    .unwrap_err();
    assert_eq!(code(&err), PERMISSION_DENIED, "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn update_and_delete_cannot_reach_another_tenants_rows() {
    let Some(db) = setup().await else { return };
    let (_, tenants) = db.service_pool().await;
    let a = seed_tenant(&tenants, "firm-a", 1).await;
    let b = seed_tenant(&tenants, "firm-b", 2).await;
    let mut tx = tenants.begin(a).await.unwrap();
    let updated = sqlx::query("UPDATE identity.users SET status = 'disabled' WHERE tenant_id = $1")
        .bind(b.as_uuid())
        .execute(tx.conn())
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(updated, 0);
    let deleted = sqlx::query("DELETE FROM identity.user_roles WHERE tenant_id = $1")
        .bind(b.as_uuid())
        .execute(tx.conn())
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(deleted, 0);
    tx.commit().await.unwrap();
    // B is untouched.
    let mut tx = tenants.begin(b).await.unwrap();
    let disabled: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.users WHERE status = 'disabled'")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    assert_eq!(disabled, 0);
    assert_eq!(count(&mut tx, "user_roles").await, 2);
    db.cleanup().await;
}

#[tokio::test]
async fn a_row_can_never_reference_a_user_of_another_tenant() {
    let Some(db) = setup().await else { return };
    let (_, tenants) = db.service_pool().await;
    let a = seed_tenant(&tenants, "firm-a", 1).await;
    let b = seed_tenant(&tenants, "firm-b", 1).await;
    // B's user id, read as B.
    let mut tx = tenants.begin(b).await.unwrap();
    let b_user: Uuid = sqlx::query_scalar("SELECT id FROM identity.users")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    drop(tx);
    // As A, create a key that points at B's user. RLS allows the row (tenant_id = A); the foreign key does not.
    let mut tx = tenants.begin(a).await.unwrap();
    let a_user: Uuid = sqlx::query_scalar("SELECT id FROM identity.users")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    let err = sqlx::query(
        "INSERT INTO identity.api_keys (tenant_id, id, user_id, name, key_prefix, key_hash, scopes, created_by) \
         VALUES ($1, $2, $3, 'k', 'AAAAAAAA', $4, ARRAY['time:read'], $5)",
    )
    .bind(a.as_uuid())
    .bind(Uuid::new_v4())
    .bind(b_user)
    .bind([9u8; 32].as_slice())
    .bind(a_user)
    .execute(tx.conn())
    .await
    .unwrap_err();
    assert_eq!(code(&err), FK_VIOLATION, "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn the_service_role_has_only_the_privileges_the_migration_grants() {
    let Some(db) = setup().await else { return };
    let (_, tenants) = db.service_pool().await;
    let a = seed_tenant(&tenants, "firm-a", 1).await;

    // A key's hash, scopes, owner and expiry are immutable: one statement each, each in its own
    // transaction because a failed statement poisons the transaction.
    for bad in [
        "UPDATE identity.api_keys SET key_hash = '\\x00'::bytea",
        "UPDATE identity.api_keys SET scopes = ARRAY['users:manage']",
        "UPDATE identity.api_keys SET expires_at = now() + interval '1 year'",
        "UPDATE identity.tenants SET slug = 'renamed'",
        "UPDATE identity.users SET email = 'other@firm-a.example'",
        "DELETE FROM identity.api_keys",
        "DELETE FROM identity.users",
        "DELETE FROM identity.tenants",
        "TRUNCATE identity.user_roles",
    ] {
        let mut tx = tenants.begin(a).await.unwrap();
        let err = sqlx::query(bad).execute(tx.conn()).await.unwrap_err();
        assert_eq!(code(&err), PERMISSION_DENIED, "{bad}: {err}");
    }

    // What is allowed.
    let mut tx = tenants.begin(a).await.unwrap();
    let touched = sqlx::query("UPDATE identity.api_keys SET last_used_at = now()")
        .execute(tx.conn())
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(touched, 1);
    let removed = sqlx::query("DELETE FROM identity.user_roles")
        .execute(tx.conn())
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(removed, 1);
    db.cleanup().await;
}

#[test]
fn tenants_never_see_each_others_rows() {
    // Property: for any number of firms with any number of users, a transaction acting for one firm
    // sees exactly that firm's rows in every table, and a transaction acting for an unknown firm sees none.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let Some(db) = rt.block_on(setup()) else { return };
    let (_, tenants) = rt.block_on(db.service_pool());
    let mut runner = TestRunner::new(Config {
        cases: 24,
        ..Config::default()
    });
    let strategy = prop::collection::vec(0usize..5, 2..5);
    let result = runner.run(&strategy, |sizes| {
        rt.block_on(async {
            let mut firms: Vec<(TenantId, usize)> = Vec::new();
            for (i, &n) in sizes.iter().enumerate() {
                let slug = format!("firm-{}-{i}", Uuid::new_v4().simple());
                firms.push((seed_tenant(&tenants, &slug, n).await, n));
            }
            for &(tenant, n) in &firms {
                let mut tx = tenants.begin(tenant).await.unwrap();
                prop_assert_eq!(count(&mut tx, "tenants").await, 1);
                prop_assert_eq!(count(&mut tx, "users").await, n as i64);
                prop_assert_eq!(count(&mut tx, "user_roles").await, n as i64);
                prop_assert_eq!(count(&mut tx, "api_keys").await, n as i64);
                // Every visible row carries this firm's id and nobody else's.
                for table in ["users", "user_roles", "api_keys"] {
                    let foreign: i64 = sqlx::query_scalar(&format!(
                        "SELECT count(*) FROM identity.{table} WHERE tenant_id <> $1"
                    ))
                    .bind(tenant.as_uuid())
                    .fetch_one(tx.conn())
                    .await
                    .unwrap();
                    prop_assert_eq!(foreign, 0, "{} showed another firm's rows", table);
                }
            }
            let mut stranger = tenants.begin(TenantId::from_uuid(Uuid::new_v4())).await.unwrap();
            for table in ["tenants", "users", "user_roles", "api_keys"] {
                prop_assert_eq!(count(&mut stranger, table).await, 0);
            }
            Ok(())
        })
    });
    result.unwrap();
    rt.block_on(db.cleanup());
}

#[test]
fn rust_validation_and_database_constraints_agree() {
    // Property: whatever the Rust rules accept, the database CHECK constraints accept too (so a
    // validated value can never fail on insert). For slugs the rules are identical, so they must
    // agree exactly in both directions.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let Some(db) = rt.block_on(setup()) else { return };
    let (_, tenants) = rt.block_on(db.service_pool());
    let firm = rt.block_on(seed_tenant(&tenants, "agreement", 0));
    let rust_accepted_emails = AtomicUsize::new(0);
    let rust_accepted_slugs = AtomicUsize::new(0);

    let slug_strategy = prop_oneof![
        "[a-z0-9][a-z0-9-]{1,38}[a-z0-9]",
        "[a-zA-Z0-9_ .-]{0,45}",
        "\\PC{0,12}"
    ];
    let email_strategy = prop_oneof![
        "[a-z0-9._+-]{1,10}@[a-z0-9-]{1,8}\\.[a-z]{2,4}",
        "[ A-Za-z0-9@._+-]{0,40}",
        "\\PC{0,20}",
    ];
    let mut runner = TestRunner::new(Config {
        cases: 200,
        ..Config::default()
    });

    runner
        .run(&slug_strategy, |slug| {
            let rust_ok = TenantSlug::parse(&slug).is_ok();
            if rust_ok {
                rust_accepted_slugs.fetch_add(1, Ordering::Relaxed);
            }
            let db_ok = rt.block_on(async {
                // A fresh tenant context each time; the transaction is dropped, never committed.
                let id = TenantId::from_uuid(Uuid::new_v4());
                let mut tx = tenants.begin(id).await.unwrap();
                match sqlx::query(
                    "INSERT INTO identity.tenants (id, slug, display_name) VALUES ($1, $2, 'x')",
                )
                .bind(id.as_uuid())
                .bind(&slug)
                .execute(tx.conn())
                .await
                {
                    Ok(_) => true,
                    Err(e) if code(&e) == CHECK_VIOLATION => false,
                    Err(e) => panic!("unexpected database error for slug {slug:?}: {e}"),
                }
            });
            prop_assert_eq!(rust_ok, db_ok, "slug {:?}", slug);
            Ok(())
        })
        .unwrap();

    // A TestRunner remembers how many cases it has passed, so the second property needs its own.
    let mut email_runner = TestRunner::new(Config {
        cases: 200,
        ..Config::default()
    });
    email_runner
        .run(&email_strategy, |raw| {
            let Ok(email) = Email::parse(&raw) else { return Ok(()) };
            rust_accepted_emails.fetch_add(1, Ordering::Relaxed);
            let db_ok = rt.block_on(async {
                let mut tx = tenants.begin(firm).await.unwrap();
                let r = sqlx::query(
                    "INSERT INTO identity.users (tenant_id, id, email, display_name) VALUES ($1, $2, $3, 'x')",
                )
                .bind(firm.as_uuid())
                .bind(Uuid::new_v4())
                .bind(email.as_str())
                .execute(tx.conn())
                .await;
                r.is_ok()
            });
            prop_assert!(db_ok, "database rejected an address Rust accepted: {:?}", email.as_str());
            Ok::<(), TestCaseError>(())
        })
        .unwrap();

    // The property above would pass vacuously if the generators never produced a valid value.
    assert!(
        rust_accepted_slugs.load(Ordering::Relaxed) > 10,
        "slug generator produced too few valid values"
    );
    assert!(
        rust_accepted_emails.load(Ordering::Relaxed) > 10,
        "email generator produced too few valid values"
    );
    rt.block_on(db.cleanup());
}
