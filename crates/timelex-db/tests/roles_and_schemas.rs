// Written by Kone Tshivhinda & Claude
//! P1-01 acceptance: "a role cannot read other schemas", plus the other promises of the bootstrap SQL.

mod common;

use common::{code, setup};
use timelex_db::bootstrap;

const PERMISSION_DENIED: &str = "42501";

#[tokio::test]
async fn bootstrap_can_be_run_twice() {
    let Some(db) = setup().await else { return };
    bootstrap(&db.admin)
        .await
        .expect("second run must succeed (idempotent)");
    db.cleanup().await;
}

#[tokio::test]
async fn service_and_migrator_roles_have_no_special_powers() {
    let Some(db) = setup().await else { return };
    let rows: Vec<(String, bool, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT rolname::text, rolsuper, rolcreaterole, rolcreatedb, rolreplication, rolbypassrls \
         FROM pg_roles WHERE rolname IN ('svc_identity', 'timelex_migrator')",
    )
    .fetch_all(&db.admin)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    for (name, su, createrole, createdb, repl, bypass) in rows {
        assert!(
            !su && !createrole && !createdb && !repl && !bypass,
            "{name} has a special power"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_service_role_cannot_read_another_services_schema() {
    let Some(db) = setup().await else { return };
    db.provision("vault").await;
    // The migrator (not a superuser) creates a table in the vault schema, as vault's migration would.
    let migrator = db.as_role("timelex_migrator").await;
    sqlx::raw_sql("CREATE TABLE vault.secrets (tenant_id uuid, note text)")
        .execute(&migrator)
        .await
        .unwrap();

    let identity = db.as_role("svc_identity").await;
    let err = sqlx::query("SELECT * FROM vault.secrets")
        .fetch_all(&identity)
        .await
        .unwrap_err();
    assert_eq!(code(&err), PERMISSION_DENIED, "{err}");

    // And the other way round: vault cannot read identity.
    let vault = db.as_role("svc_vault").await;
    let err = sqlx::query("SELECT * FROM identity.tenants")
        .fetch_all(&vault)
        .await
        .unwrap_err();
    assert_eq!(code(&err), PERMISSION_DENIED, "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn a_service_role_cannot_create_objects_anywhere() {
    let Some(db) = setup().await else { return };
    let identity = db.as_role("svc_identity").await;
    for ddl in [
        "CREATE TABLE identity.sneaky (id int)",
        "CREATE TABLE public.sneaky (id int)",
        "CREATE SCHEMA sneaky",
    ] {
        let err = sqlx::raw_sql(ddl).execute(&identity).await.unwrap_err();
        assert_eq!(code(&err), PERMISSION_DENIED, "{ddl}: {err}");
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_service_role_cannot_touch_the_platform_tables_or_call_admin_procedures() {
    let Some(db) = setup().await else { return };
    let identity = db.as_role("svc_identity").await;
    let err = sqlx::query("CALL platform.provision_service('another')")
        .execute(&identity)
        .await
        .unwrap_err();
    assert_eq!(code(&err), PERMISSION_DENIED, "{err}");
    let err = sqlx::query("CALL platform.apply_tenant_rls('identity.users')")
        .execute(&identity)
        .await
        .unwrap_err();
    assert_eq!(code(&err), PERMISSION_DENIED, "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn invalid_service_names_are_rejected() {
    let Some(db) = setup().await else { return };
    for name in [
        "pg_evil",
        "platform",
        "public",
        "Bad",
        "a-b",
        "x",
        "1abc",
        "a b",
        "",
        "has;semicolon",
    ] {
        let err = sqlx::query("CALL platform.provision_service($1)")
            .bind(name)
            .execute(&db.admin)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid service name"),
            "{name:?}: {err}"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_provisioned_role_can_use_only_its_own_schema_and_the_search_path_points_there() {
    let Some(db) = setup().await else { return };
    let identity = db.as_role("svc_identity").await;
    let (sp,): (String,) = sqlx::query_as("SHOW search_path")
        .fetch_one(&identity)
        .await
        .unwrap();
    assert_eq!(sp, "identity");
    let (schemas,): (Vec<String>,) = sqlx::query_as(
        "SELECT coalesce(array_agg(nspname::text ORDER BY nspname), '{}') FROM pg_namespace \
         WHERE has_schema_privilege(current_user, oid, 'USAGE') AND nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema'",
    )
    .fetch_one(&identity)
    .await
    .unwrap();
    // identity (its own) and platform (only to call current_tenant()); nothing else.
    assert_eq!(schemas, vec!["identity".to_string(), "platform".to_string()]);
    db.cleanup().await;
}
