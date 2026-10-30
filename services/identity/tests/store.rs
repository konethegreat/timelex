// Written by Kone Tshivhinda & Claude
//! P1-03 acceptance: create a firm, issue a key, authenticate. Plus the rules around them, run against
//! a real Postgres with the production roles and row-level security.

use std::sync::{Arc, Mutex};

use time::{Duration, OffsetDateTime};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_db::TenantPool;
use timelex_identity::{
    ApiKeyId, AuthFailure, IdentityError, NewTenantRequest, Permission, Role, RoleSet, TenantId, TokenError,
    UserId,
};
use timelex_identity_service::{
    AuthError, Clock, CreatedTenant, IdentityStore, IssueKeyRequest, Principal, StoreError, MIGRATIONS,
};
use uuid::Uuid;

#[derive(Clone)]
struct TestClock(Arc<Mutex<OffsetDateTime>>);

impl TestClock {
    fn new() -> Self {
        // Whole seconds: Postgres keeps microseconds, and equality checks stay simple.
        let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
        Self(Arc::new(Mutex::new(now)))
    }
    fn clock(&self) -> Clock {
        let t = self.0.clone();
        Arc::new(move || *t.lock().unwrap())
    }
    fn now(&self) -> OffsetDateTime {
        *self.0.lock().unwrap()
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

struct Env {
    db: TestDb,
    store: IdentityStore,
    tenants: TenantPool,
    clock: TestClock,
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "identity",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let clock = TestClock::new();
    let store = IdentityStore::with_clock(tenants.clone(), clock.clock());
    Some(Env {
        db,
        store,
        tenants,
        clock,
    })
}

fn firm<'a>(slug: &'a str, email: &'a str) -> NewTenantRequest<'a> {
    NewTenantRequest {
        slug,
        display_name: "Example Attorneys Inc",
        admin_email: email,
        admin_display_name: "Pat Partner",
    }
}

async fn admin_principal(env: &Env, created: &CreatedTenant) -> Principal {
    env.store
        .authenticate(created.initial_key.token.expose_secret())
        .await
        .unwrap()
}

/// Insert a user directly (there is no create-user operation yet), acting as the firm.
async fn seed_user(
    env: &Env,
    tenant: TenantId,
    email: &str,
    roles: &[&str],
    status: &str,
    granted_by: UserId,
) -> UserId {
    let id = Uuid::new_v4();
    let mut tx = env.tenants.begin(tenant).await.unwrap();
    sqlx::query("INSERT INTO identity.users (tenant_id, id, email, display_name, status) VALUES ($1, $2, $3, 'Seeded', $4)")
        .bind(tenant.as_uuid())
        .bind(id)
        .bind(email)
        .bind(status)
        .execute(tx.conn())
        .await
        .unwrap();
    for role in roles {
        sqlx::query(
            "INSERT INTO identity.user_roles (tenant_id, user_id, role, granted_by) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant.as_uuid())
        .bind(id)
        .bind(role)
        .bind(granted_by.as_uuid())
        .execute(tx.conn())
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    UserId::from_uuid(id)
}

async fn run_as_firm(env: &Env, tenant: TenantId, sql: &str) {
    let mut tx = env.tenants.begin(tenant).await.unwrap();
    // Test fixtures only: callers pass a literal or a statement built from a typed UUID.
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

fn key_request(scopes: &[Permission]) -> IssueKeyRequest {
    IssueKeyRequest {
        owner: None,
        name: "test key".into(),
        scopes: scopes.to_vec(),
        expires_at: None,
    }
}

// ---------- the acceptance path ----------

#[tokio::test]
async fn create_a_firm_then_authenticate_with_its_first_key() {
    let Some(env) = env().await else { return };
    let created = env
        .store
        .create_tenant(&firm("example-attorneys", "Partner@Example-Attorneys.example"))
        .await
        .unwrap();
    assert_eq!(created.slug.as_str(), "example-attorneys");
    assert_eq!(created.initial_key.token.tenant(), created.tenant_id);

    let p = admin_principal(&env, &created).await;
    assert_eq!(p.tenant, created.tenant_id);
    assert_eq!(p.user, created.admin_user_id);
    assert_eq!(p.key, created.initial_key.id);
    assert_eq!(p.roles, RoleSet::new([Role::Admin]).unwrap());
    assert_eq!(p.permissions, Role::Admin.permissions().iter().copied().collect());
    env.db.cleanup().await;
}

#[tokio::test]
async fn an_admin_can_issue_a_key_and_that_key_authenticates() {
    let Some(env) = env().await else { return };
    let created = env
        .store
        .create_tenant(&firm("example-attorneys", "partner@example-attorneys.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &created).await;

    let issued = env
        .store
        .issue_api_key(&admin, key_request(&[Permission::TimeRead]))
        .await
        .unwrap();
    assert_eq!(issued.user_id, admin.user, "defaults to the caller");
    let p = env
        .store
        .authenticate(issued.token.expose_secret())
        .await
        .unwrap();
    assert_eq!(p.key, issued.id);
    assert_eq!(
        p.permissions,
        [Permission::TimeRead].into_iter().collect(),
        "a narrow key stays narrow"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn the_secret_is_never_stored() {
    let Some(env) = env().await else { return };
    let created = env
        .store
        .create_tenant(&firm("example-attorneys", "partner@example-attorneys.example"))
        .await
        .unwrap();
    let token = created.initial_key.token.expose_secret().to_owned();
    let secret_part = token.split('.').nth(1).unwrap().to_owned();

    // Superuser view of the whole row, as JSON text: the secret must appear nowhere in it.
    let rows: Vec<String> = sqlx::query_scalar("SELECT row_to_json(k)::text FROM identity.api_keys k")
        .fetch_all(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].contains(&secret_part), "secret found in the stored row");
    assert!(!rows[0].contains(&token), "token found in the stored row");
    let stored: Vec<u8> = sqlx::query_scalar("SELECT key_hash FROM identity.api_keys")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(
        stored.as_slice(),
        created.initial_key.token.hash().as_bytes().as_slice()
    );
    env.db.cleanup().await;
}

// ---------- creating firms ----------

#[tokio::test]
async fn a_slug_can_only_be_used_once_and_nothing_is_left_behind() {
    let Some(env) = env().await else { return };
    env.store
        .create_tenant(&firm("example-attorneys", "a@one.example"))
        .await
        .unwrap();
    let err = env
        .store
        .create_tenant(&firm("example-attorneys", "b@two.example"))
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::SlugTaken), "{err}");
    let (tenants, users, keys): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM identity.tenants), (SELECT count(*) FROM identity.users), (SELECT count(*) FROM identity.api_keys)",
    )
    .fetch_one(&env.db.admin)
    .await
    .unwrap();
    assert_eq!(
        (tenants, users, keys),
        (1, 1, 1),
        "the failed attempt left rows behind"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn invalid_firm_requests_are_rejected_before_touching_the_database() {
    let Some(env) = env().await else { return };
    let err = env
        .store
        .create_tenant(&firm("Bad Slug", "a@one.example"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::Rule(IdentityError::InvalidSlug)),
        "{err}"
    );
    let err = env
        .store
        .create_tenant(&firm("good-slug", "not-an-email"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::Rule(IdentityError::InvalidEmail)),
        "{err}"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.tenants")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(n, 0);
    env.db.cleanup().await;
}

// ---------- authenticating ----------

#[tokio::test]
async fn bad_tokens_are_refused_and_each_reason_is_distinguishable_internally() {
    let Some(env) = env().await else { return };
    let a = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let b = env
        .store
        .create_tenant(&firm("firm-b", "b@two.example"))
        .await
        .unwrap();
    let good = a.initial_key.token.expose_secret().to_owned();

    assert!(matches!(
        env.store.authenticate("").await,
        Err(AuthError::Malformed(TokenError::BadLength))
    ));
    assert!(matches!(
        env.store.authenticate("not a token").await,
        Err(AuthError::Malformed(_))
    ));

    // Well-formed, right firm, wrong secret.
    let mut wrong_secret = good.clone();
    wrong_secret.replace_range(50..51, if &good[50..51] == "A" { "B" } else { "A" });
    assert!(matches!(
        env.store.authenticate(&wrong_secret).await,
        Err(AuthError::UnknownKey)
    ));

    // A real secret presented with another firm's prefix finds nothing: the tenant part is a claim, not proof.
    let swapped = good.replacen(&a.tenant_id.to_hex32(), &b.tenant_id.to_hex32(), 1);
    assert!(matches!(
        env.store.authenticate(&swapped).await,
        Err(AuthError::UnknownKey)
    ));

    // A firm that does not exist.
    let ghost = good.replacen(
        &a.tenant_id.to_hex32(),
        &TenantId::from_uuid(Uuid::new_v4()).to_hex32(),
        1,
    );
    assert!(matches!(
        env.store.authenticate(&ghost).await,
        Err(AuthError::UnknownKey)
    ));

    // The genuine key still works afterwards.
    assert!(env.store.authenticate(&good).await.is_ok());
    env.db.cleanup().await;
}

#[tokio::test]
async fn a_suspended_firm_cannot_authenticate() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    run_as_firm(
        &env,
        c.tenant_id,
        "UPDATE identity.tenants SET status = 'suspended'",
    )
    .await;
    let err = env
        .store
        .authenticate(c.initial_key.token.expose_secret())
        .await
        .unwrap_err();
    assert!(
        matches!(err, AuthError::Rejected(AuthFailure::TenantSuspended)),
        "{err}"
    );
    run_as_firm(&env, c.tenant_id, "UPDATE identity.tenants SET status = 'active'").await;
    assert!(env
        .store
        .authenticate(c.initial_key.token.expose_secret())
        .await
        .is_ok());
    env.db.cleanup().await;
}

#[tokio::test]
async fn a_disabled_owner_blocks_their_keys() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    run_as_firm(&env, c.tenant_id, "UPDATE identity.users SET status = 'disabled'").await;
    let err = env
        .store
        .authenticate(c.initial_key.token.expose_secret())
        .await
        .unwrap_err();
    assert!(
        matches!(err, AuthError::Rejected(AuthFailure::UserNotActive)),
        "{err}"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn keys_expire_at_their_expiry_time() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &c).await;
    let mut req = key_request(&[Permission::TimeRead]);
    req.expires_at = Some(env.clock.now() + Duration::hours(1));
    let issued = env.store.issue_api_key(&admin, req).await.unwrap();

    env.clock.advance(Duration::minutes(59));
    assert!(env.store.authenticate(issued.token.expose_secret()).await.is_ok());
    env.clock.advance(Duration::minutes(1));
    let err = env
        .store
        .authenticate(issued.token.expose_secret())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Rejected(AuthFailure::Expired)), "{err}");
    env.db.cleanup().await;
}

#[tokio::test]
async fn removing_a_role_shrinks_existing_keys_at_once() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &c).await;
    let user = seed_user(
        &env,
        c.tenant_id,
        "both@one.example",
        &["fee_earner", "billing"],
        "active",
        admin.user,
    )
    .await;
    let mut req = key_request(&[Permission::TimeWrite, Permission::BillingWrite]);
    req.owner = Some(user);
    let issued = env.store.issue_api_key(&admin, req).await.unwrap();

    let before = env
        .store
        .authenticate(issued.token.expose_secret())
        .await
        .unwrap();
    assert_eq!(
        before.permissions,
        [Permission::TimeWrite, Permission::BillingWrite]
            .into_iter()
            .collect()
    );

    run_as_firm(
        &env,
        c.tenant_id,
        &format!(
            "DELETE FROM identity.user_roles WHERE user_id = '{}' AND role = 'billing'",
            user.as_uuid()
        ),
    )
    .await;
    let after = env
        .store
        .authenticate(issued.token.expose_secret())
        .await
        .unwrap();
    assert_eq!(after.permissions, [Permission::TimeWrite].into_iter().collect());
    env.db.cleanup().await;
}

#[tokio::test]
async fn last_used_is_recorded_but_not_on_every_request() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let token = c.initial_key.token.expose_secret().to_owned();
    let read = |db: &TestDb| {
        let pool = db.admin.clone();
        async move {
            sqlx::query_scalar::<_, Option<OffsetDateTime>>("SELECT last_used_at FROM identity.api_keys")
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(read(&env.db).await, None);
    env.store.authenticate(&token).await.unwrap();
    let first = read(&env.db).await.expect("recorded on first use");
    env.clock.advance(Duration::seconds(30));
    env.store.authenticate(&token).await.unwrap();
    assert_eq!(read(&env.db).await, Some(first), "refreshed too soon");
    env.clock.advance(Duration::seconds(31));
    env.store.authenticate(&token).await.unwrap();
    assert_eq!(
        read(&env.db).await,
        Some(env.clock.now()),
        "should refresh after a minute"
    );
    env.db.cleanup().await;
}

// ---------- issuing keys ----------

#[tokio::test]
async fn issuing_is_refused_for_the_wrong_caller_owner_or_scopes() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &c).await;
    let agent = seed_user(
        &env,
        c.tenant_id,
        "bot@one.example",
        &["agent"],
        "active",
        admin.user,
    )
    .await;
    let invited = seed_user(
        &env,
        c.tenant_id,
        "new@one.example",
        &["fee_earner"],
        "invited",
        admin.user,
    )
    .await;
    let fee_earner = seed_user(
        &env,
        c.tenant_id,
        "fe@one.example",
        &["fee_earner"],
        "active",
        admin.user,
    )
    .await;

    // An agent key within its role is fine.
    let ok = IssueKeyRequest {
        owner: Some(agent),
        ..key_request(&[Permission::CaptureWrite])
    };
    assert!(env.store.issue_api_key(&admin, ok).await.is_ok());

    // Human-only scope for a machine: refused.
    let bad = IssueKeyRequest {
        owner: Some(agent),
        ..key_request(&[Permission::KeysManage])
    };
    let err = env.store.issue_api_key(&admin, bad).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Rule(IdentityError::ScopeNotGranted(_))),
        "{err}"
    );

    // Invited owner: refused.
    let req = IssueKeyRequest {
        owner: Some(invited),
        ..key_request(&[Permission::TimeRead])
    };
    let err = env.store.issue_api_key(&admin, req).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Rule(IdentityError::KeyOwnerNotActive)),
        "{err}"
    );

    // Unknown owner: not found.
    let req = IssueKeyRequest {
        owner: Some(UserId::from_uuid(Uuid::new_v4())),
        ..key_request(&[Permission::TimeRead])
    };
    assert!(matches!(
        env.store.issue_api_key(&admin, req).await,
        Err(StoreError::NotFound)
    ));

    // Empty scopes, past expiry.
    let err = env
        .store
        .issue_api_key(&admin, key_request(&[]))
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Rule(IdentityError::NoScopes)), "{err}");
    let mut req = key_request(&[Permission::TimeRead]);
    req.expires_at = Some(env.clock.now() - Duration::seconds(1));
    let err = env.store.issue_api_key(&admin, req).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Rule(IdentityError::ExpiryInPast)),
        "{err}"
    );

    // A fee earner (no keys:manage) cannot issue, not even for themselves.
    let fe_key = env
        .store
        .issue_api_key(
            &admin,
            IssueKeyRequest {
                owner: Some(fee_earner),
                ..key_request(&[Permission::TimeRead, Permission::TimeWrite])
            },
        )
        .await
        .unwrap();
    let fe = env
        .store
        .authenticate(fe_key.token.expose_secret())
        .await
        .unwrap();
    let err = env
        .store
        .issue_api_key(&fe, key_request(&[Permission::TimeRead]))
        .await
        .unwrap_err();
    assert!(err.is_forbidden(), "{err}");
    env.db.cleanup().await;
}

#[tokio::test]
async fn one_firm_cannot_issue_keys_for_or_revoke_keys_of_another() {
    let Some(env) = env().await else { return };
    let a = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let b = env
        .store
        .create_tenant(&firm("firm-b", "b@two.example"))
        .await
        .unwrap();
    let admin_a = admin_principal(&env, &a).await;

    // B's user, B's key: as A they look like they do not exist.
    let req = IssueKeyRequest {
        owner: Some(b.admin_user_id),
        ..key_request(&[Permission::TimeRead])
    };
    assert!(matches!(
        env.store.issue_api_key(&admin_a, req).await,
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        env.store.revoke_api_key(&admin_a, b.initial_key.id).await,
        Err(StoreError::NotFound)
    ));
    // B's key is untouched.
    assert!(env
        .store
        .authenticate(b.initial_key.token.expose_secret())
        .await
        .is_ok());
    env.db.cleanup().await;
}

// ---------- revoking keys ----------

#[tokio::test]
async fn revoking_takes_effect_at_once_and_is_repeatable() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &c).await;
    let issued = env
        .store
        .issue_api_key(&admin, key_request(&[Permission::TimeRead]))
        .await
        .unwrap();
    assert!(env.store.authenticate(issued.token.expose_secret()).await.is_ok());

    env.store.revoke_api_key(&admin, issued.id).await.unwrap();
    let err = env
        .store
        .authenticate(issued.token.expose_secret())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Rejected(AuthFailure::Revoked)), "{err}");
    env.store
        .revoke_api_key(&admin, issued.id)
        .await
        .expect("revoking twice is fine");
    assert!(matches!(
        env.store
            .revoke_api_key(&admin, ApiKeyId::from_uuid(Uuid::new_v4()))
            .await,
        Err(StoreError::NotFound)
    ));
    env.db.cleanup().await;
}

#[tokio::test]
async fn revoking_needs_keys_manage() {
    let Some(env) = env().await else { return };
    let c = env
        .store
        .create_tenant(&firm("firm-a", "a@one.example"))
        .await
        .unwrap();
    let admin = admin_principal(&env, &c).await;
    let user = seed_user(
        &env,
        c.tenant_id,
        "fe@one.example",
        &["fee_earner"],
        "active",
        admin.user,
    )
    .await;
    let key = env
        .store
        .issue_api_key(
            &admin,
            IssueKeyRequest {
                owner: Some(user),
                ..key_request(&[Permission::TimeRead])
            },
        )
        .await
        .unwrap();
    let fe = env.store.authenticate(key.token.expose_secret()).await.unwrap();
    let err = env.store.revoke_api_key(&fe, key.id).await.unwrap_err();
    assert!(err.is_forbidden(), "{err}");
    assert!(
        env.store.authenticate(key.token.expose_secret()).await.is_ok(),
        "must not have been revoked"
    );
    env.db.cleanup().await;
}

// ---------- the contract ----------

#[test]
fn the_openapi_contract_names_the_same_operations_roles_and_permissions_as_the_code() {
    // A cheap drift guard until P1-09 validates the contract properly: the enumerations in the YAML
    // must match the Rust vocabulary exactly, and every operation we implement must be present.
    let yaml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/openapi/identity.v1.yaml"
    ))
    .expect("contract file");
    for op in ["createTenant", "issueApiKey", "revokeApiKey", "authenticate"] {
        assert!(
            yaml.contains(&format!("operationId: {op}")),
            "contract lacks {op}"
        );
    }
    let roles: Vec<&str> = Role::ALL.iter().map(Role::as_str).collect();
    assert!(
        yaml.contains(&format!("enum: [{}]", roles.join(", "))),
        "Role enum drifted"
    );
    let perms: Vec<&str> = Permission::ALL.iter().map(Permission::as_str).collect();
    assert!(
        yaml.contains(&format!("enum: [{}]", perms.join(", "))),
        "Permission enum drifted"
    );
    assert!(
        yaml.contains(r#"pattern: "^[a-z0-9][a-z0-9-]{1,38}[a-z0-9]$""#),
        "slug pattern drifted"
    );
    assert!(yaml.contains("minLength: 80"), "token length drifted");
}
