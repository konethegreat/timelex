// Written by Kone Tshivhinda & Claude
//! P1-05 acceptance: a secret is stored sealed, listed as metadata, rotated, revoked and audited,
//! against a real Postgres with the production roles and row-level security.
//!
//! The provider call is not here: it is an HTTP client and belongs to the service's next part. These
//! tests stop at `record_test`, which is what the service does with the result.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use sqlx::Row;
use time::{Duration, OffsetDateTime};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_db::TenantPool;
use timelex_identity::{Permission, TenantId, UserId};
use timelex_vault::{entry_hash, AuditAction, AuditRecord, LocalKeyWrapper, SecretId, SecretKind, GENESIS};
use timelex_vault_service::{
    Actor, Clock, PutOutcome, PutSecretRequest, StoreError, TestOutcome, VaultStore, MIGRATIONS,
};
use uuid::Uuid;

const ANTHROPIC_V1: &str = "sk-ant-api03-FIRSTvalue00001111";
const ANTHROPIC_V2: &str = "sk-ant-api03-SECONDvalue009999";

#[derive(Clone)]
struct TestClock(Arc<Mutex<OffsetDateTime>>);

impl TestClock {
    fn new() -> Self {
        // Whole microseconds, which is Postgres's precision and the audit hash's unit.
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
    store: VaultStore,
    tenants: TenantPool,
    clock: TestClock,
}

/// A fresh master key per test, so no key is a literal in this repository.
fn random_key() -> [u8; 32] {
    let bytes: Vec<u8> = (0..2).flat_map(|_| Uuid::new_v4().into_bytes()).collect();
    std::array::from_fn(|i| bytes[i])
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "vault",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let clock = TestClock::new();
    let wrapper = Arc::new(LocalKeyWrapper::new(random_key()));
    let store = VaultStore::with_clock(tenants.clone(), wrapper, clock.clock());
    Some(Env {
        db,
        store,
        tenants,
        clock,
    })
}

fn actor(tenant: TenantId, permissions: &[Permission]) -> Actor {
    Actor {
        tenant,
        user: UserId::from_uuid(Uuid::new_v4()),
        permissions: permissions.iter().copied().collect::<BTreeSet<_>>(),
    }
}

fn settings(tenant: TenantId) -> Actor {
    actor(tenant, &[Permission::SettingsManage])
}

fn request(value: &str) -> PutSecretRequest {
    PutSecretRequest {
        label: "Firm Claude key".into(),
        value: value.into(),
        allowed_hosts: Vec::new(),
    }
}

/// Append-only audit chain verification: seqs are 1..n, each `prev_hash` is the hash before it, each
/// `entry_hash` is what the domain crate computes, and the head names the last entry.
async fn assert_chain_verifies(db: &TestDb, tenant: TenantId) {
    let rows = sqlx::query(
        "SELECT seq, at, action, actor_id, secret_id, detail, prev_hash, entry_hash \
         FROM vault.secret_audit WHERE tenant_id = $1 ORDER BY seq",
    )
    .bind(tenant.as_uuid())
    .fetch_all(&db.admin)
    .await
    .unwrap();
    let mut prev = GENESIS;
    for (i, row) in rows.iter().enumerate() {
        let seq: i64 = row.get("seq");
        assert_eq!(seq, i as i64 + 1, "sequence numbers are contiguous");
        let at: OffsetDateTime = row.get("at");
        let action: String = row.get("action");
        let detail: String = row.get("detail");
        let record = AuditRecord {
            seq,
            at_micros: (at.unix_timestamp_nanos() / 1_000) as i64,
            action: AuditAction::parse(&action).expect("stored action is known"),
            actor: UserId::from_uuid(row.get("actor_id")),
            secret: SecretId::from_uuid(row.get("secret_id")),
            detail: &detail,
        };
        assert_eq!(
            row.get::<Vec<u8>, _>("prev_hash"),
            prev.to_vec(),
            "entry {seq} links"
        );
        let hash = entry_hash(&prev, &record);
        assert_eq!(
            row.get::<Vec<u8>, _>("entry_hash"),
            hash.to_vec(),
            "entry {seq} hashes"
        );
        prev = hash;
    }
    let (head_seq, head_hash): (i64, Vec<u8>) =
        sqlx::query_as("SELECT seq, hash FROM vault.chain_heads WHERE tenant_id = $1")
            .bind(tenant.as_uuid())
            .fetch_one(&db.admin)
            .await
            .unwrap();
    assert_eq!(head_seq, rows.len() as i64);
    assert_eq!(head_hash, prev.to_vec());
}

// ---------- the acceptance path ----------

#[tokio::test]
async fn a_secret_is_stored_sealed_and_only_ever_read_back_as_metadata() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let outcome = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap();
    let PutOutcome::Created(meta) = outcome else {
        panic!("a first put creates");
    };
    assert_eq!(meta.kind, SecretKind::AnthropicApiKey);
    assert_eq!(meta.label, "Firm Claude key");
    assert_eq!(meta.last_four, "1111");
    assert_eq!(meta.allowed_hosts, ["api.anthropic.com"]);
    assert_eq!(meta.revoked_at, None);

    // What the database holds: ciphertext and a wrapped key, never the value.
    let row = sqlx::query(
        "SELECT ciphertext, value_nonce, wrapped_dek, dek_nonce, key_version \
         FROM vault.secrets WHERE id = $1",
    )
    .bind(meta.id.as_uuid())
    .fetch_one(&env.db.admin)
    .await
    .unwrap();
    let ciphertext: Vec<u8> = row.get("ciphertext");
    assert!(
        !contains(&ciphertext, ANTHROPIC_V1.as_bytes()),
        "the value is not stored"
    );
    assert!(!contains(&ciphertext, b"sk-ant-api03"));
    assert_eq!(ciphertext.len(), ANTHROPIC_V1.len() + 16);
    assert_eq!(row.get::<Vec<u8>, _>("value_nonce").len(), 12);
    assert_eq!(row.get::<Vec<u8>, _>("wrapped_dek").len(), 48);
    assert_eq!(row.get::<Vec<u8>, _>("dek_nonce").len(), 12);
    assert_eq!(row.get::<String, _>("key_version"), "v1");

    // The only reads: metadata over the API, and the plaintext for a provider check.
    let listed = env.store.list(settings(tenant), false).await.unwrap();
    assert_eq!(listed, vec![meta.clone()]);
    let opened = env.store.open_for_test(settings(tenant), meta.id).await.unwrap();
    assert_eq!(opened.value.expose_secret(), ANTHROPIC_V1);
    assert!(
        !format!("{opened:?}").contains(ANTHROPIC_V1),
        "Debug hides the value"
    );

    assert_chain_verifies(&env.db, tenant).await;
}

#[tokio::test]
async fn putting_a_kind_again_replaces_the_value_and_destroys_the_old_one() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let PutOutcome::Created(first) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap()
    else {
        panic!("first put creates");
    };
    env.clock.advance(Duration::seconds(60));
    let PutOutcome::Replaced(second) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V2),
        )
        .await
        .unwrap()
    else {
        panic!("second put replaces");
    };
    assert_eq!(second.id, first.id, "the secret keeps its identity");
    assert_eq!(second.created_at, first.created_at);
    assert!(second.updated_at > first.updated_at);
    assert_eq!(second.last_four, "9999");

    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secrets WHERE revoked_at IS NULL")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(live, 1, "one live secret per kind");
    let opened = env
        .store
        .open_for_test(settings(tenant), second.id)
        .await
        .unwrap();
    assert_eq!(opened.value.expose_secret(), ANTHROPIC_V2);

    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM vault.secret_audit ORDER BY seq")
        .fetch_all(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(actions, ["created", "rotated"]);
    assert_chain_verifies(&env.db, tenant).await;
}

#[tokio::test]
async fn rotation_keeps_the_identity_and_clears_the_test_stamp() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let PutOutcome::Created(meta) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap()
    else {
        panic!("creates");
    };
    env.store
        .record_test(settings(tenant), meta.id, TestOutcome::Passed)
        .await
        .unwrap();
    env.clock.advance(Duration::minutes(5));
    let rotated = env
        .store
        .rotate(settings(tenant), meta.id, ANTHROPIC_V2.to_owned())
        .await
        .unwrap();
    assert_eq!(rotated.id, meta.id);
    assert_eq!(rotated.last_four, "9999");
    assert_eq!(rotated.last_tested_at, None, "a new value has not been tested");
    assert_eq!(rotated.last_used_at, None, "testing is not use");
    assert_eq!(rotated.created_at, meta.created_at);
    assert!(rotated.updated_at > meta.updated_at);

    let opened = env.store.open_for_test(settings(tenant), meta.id).await.unwrap();
    assert_eq!(opened.value.expose_secret(), ANTHROPIC_V2);
    assert_chain_verifies(&env.db, tenant).await;
}

#[tokio::test]
async fn testing_sets_the_test_stamp_and_never_the_used_stamp() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let PutOutcome::Created(meta) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap()
    else {
        panic!("creates");
    };
    env.store
        .record_test(settings(tenant), meta.id, TestOutcome::Failed)
        .await
        .unwrap();
    let listed = env.store.list(settings(tenant), false).await.unwrap();
    assert_eq!(listed[0].last_tested_at, Some(env.clock.now()));
    assert_eq!(listed[0].last_used_at, None);
    let details: Vec<String> =
        sqlx::query_scalar("SELECT detail FROM vault.secret_audit WHERE action = 'tested'")
            .fetch_all(&env.db.admin)
            .await
            .unwrap();
    assert_eq!(details, ["failed"]);
    assert_chain_verifies(&env.db, tenant).await;
}

#[tokio::test]
async fn revoking_destroys_the_value_and_keeps_the_metadata() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let PutOutcome::Created(meta) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap()
    else {
        panic!("creates");
    };
    env.store.revoke(settings(tenant), meta.id).await.unwrap();

    let row = sqlx::query(
        "SELECT ciphertext, value_nonce, wrapped_dek, dek_nonce, key_version, revoked_at, revoked_by, label \
         FROM vault.secrets WHERE id = $1",
    )
    .bind(meta.id.as_uuid())
    .fetch_one(&env.db.admin)
    .await
    .unwrap();
    for column in ["ciphertext", "value_nonce", "wrapped_dek", "dek_nonce"] {
        assert!(
            row.get::<Option<Vec<u8>>, _>(column).is_none(),
            "{column} still holds something"
        );
    }
    assert!(row.get::<Option<String>, _>("key_version").is_none());
    assert_eq!(row.get::<String, _>("label"), "Firm Claude key", "metadata stays");

    assert_eq!(env.store.list(settings(tenant), false).await.unwrap(), []);
    let all = env.store.list(settings(tenant), true).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].revoked_at, Some(env.clock.now()));

    assert_eq!(
        env.store.open_for_test(settings(tenant), meta.id).await.err(),
        Some(StoreError::Revoked)
    );
    assert_eq!(
        env.store
            .rotate(settings(tenant), meta.id, ANTHROPIC_V2.into())
            .await
            .err(),
        Some(StoreError::Revoked)
    );
    assert_eq!(
        env.store
            .record_test(settings(tenant), meta.id, TestOutcome::Passed)
            .await
            .err(),
        Some(StoreError::Revoked)
    );

    // Idempotent, and it does not write a second audit entry.
    env.store.revoke(settings(tenant), meta.id).await.unwrap();
    let revoked_entries: i64 =
        sqlx::query_scalar("SELECT count(*) FROM vault.secret_audit WHERE action = 'revoked'")
            .fetch_one(&env.db.admin)
            .await
            .unwrap();
    assert_eq!(revoked_entries, 1);

    // A new secret of the same kind is allowed; the revoked row stays for history.
    let PutOutcome::Created(fresh) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V2),
        )
        .await
        .unwrap()
    else {
        panic!("creates again");
    };
    assert_ne!(fresh.id, meta.id);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secrets")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(rows, 2);
    assert_chain_verifies(&env.db, tenant).await;
}

// ---------- tenancy and permission ----------

#[tokio::test]
async fn another_firm_cannot_see_or_touch_a_secret() {
    let Some(env) = env().await else { return };
    let (a, b) = (
        TenantId::from_uuid(Uuid::new_v4()),
        TenantId::from_uuid(Uuid::new_v4()),
    );
    let PutOutcome::Created(meta) = env
        .store
        .put(settings(a), SecretKind::AnthropicApiKey, request(ANTHROPIC_V1))
        .await
        .unwrap()
    else {
        panic!("creates");
    };
    assert_eq!(env.store.list(settings(b), true).await.unwrap(), []);
    for error in [
        env.store.open_for_test(settings(b), meta.id).await.err(),
        env.store
            .rotate(settings(b), meta.id, ANTHROPIC_V2.into())
            .await
            .err(),
        env.store.revoke(settings(b), meta.id).await.err(),
        env.store
            .record_test(settings(b), meta.id, TestOutcome::Passed)
            .await
            .err(),
    ] {
        assert_eq!(
            error,
            Some(StoreError::NotFound),
            "another firm's secret looks absent"
        );
    }
    // The refused calls wrote nothing at all: no audit entry for the second firm.
    let b_entries: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secret_audit WHERE tenant_id = $1")
        .bind(b.as_uuid())
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(b_entries, 0);
    let a_entries: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secret_audit WHERE tenant_id = $1")
        .bind(a.as_uuid())
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(a_entries, 1);
}

#[tokio::test]
async fn a_caller_without_settings_manage_is_refused_and_nothing_is_written() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let reader = actor(tenant, &[Permission::TimeRead]);
    assert_eq!(
        env.store
            .put(reader.clone(), SecretKind::AnthropicApiKey, request(ANTHROPIC_V1))
            .await
            .err(),
        Some(StoreError::Forbidden)
    );
    assert_eq!(
        env.store.list(reader, false).await.err(),
        Some(StoreError::Forbidden)
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secrets")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let entries: i64 = sqlx::query_scalar("SELECT count(*) FROM vault.secret_audit")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(entries, 0);
}

// ---------- the checks the database itself makes ----------

#[tokio::test]
async fn the_database_refuses_a_clear_value_a_bad_host_and_a_rewrite() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let PutOutcome::Created(meta) = env
        .store
        .put(
            settings(tenant),
            SecretKind::AnthropicApiKey,
            request(ANTHROPIC_V1),
        )
        .await
        .unwrap()
    else {
        panic!("creates");
    };

    // A row with a value but no wrapped key: refused (sealed together or not at all).
    {
        let mut tx = env.tenants.begin(tenant).await.unwrap();
        let partial = sqlx::query("UPDATE vault.secrets SET ciphertext = $2 WHERE id = $1")
            .bind(meta.id.as_uuid())
            .bind(b"plaintext".as_slice())
            .execute(tx.conn())
            .await;
        assert_eq!(timelex_db::testing::code(&partial.unwrap_err()), "23514");
    }

    // An uppercase host: refused by the coarse format check.
    {
        let mut tx = env.tenants.begin(tenant).await.unwrap();
        let hosts = sqlx::query("UPDATE vault.secrets SET allowed_hosts = $2 WHERE id = $1")
            .bind(meta.id.as_uuid())
            .bind(vec!["API.anthropic.com".to_owned()])
            .execute(tx.conn())
            .await;
        assert_eq!(timelex_db::testing::code(&hosts.unwrap_err()), "23514");
    }

    // The audit chain is append-only, and a secret is revoked rather than deleted.
    for sql in [
        "UPDATE vault.secret_audit SET detail = 'rewritten'",
        "DELETE FROM vault.secret_audit",
        "DELETE FROM vault.secrets",
    ] {
        let mut tx = env.tenants.begin(tenant).await.unwrap();
        let denied = sqlx::query(sql).execute(tx.conn()).await;
        assert_eq!(timelex_db::testing::code(&denied.unwrap_err()), "42501", "{sql}");
    }

    // A revoked secret's value cannot be put back.
    env.store.revoke(settings(tenant), meta.id).await.unwrap();
    let mut tx = env.tenants.begin(tenant).await.unwrap();
    let restore = sqlx::query(
        "UPDATE vault.secrets SET ciphertext = $2, value_nonce = $3, wrapped_dek = $4, dek_nonce = $5, key_version = 'v1' WHERE id = $1",
    )
    .bind(meta.id.as_uuid())
    .bind(vec![0u8; 32])
    .bind(vec![0u8; 12])
    .bind(vec![0u8; 48])
    .bind(vec![0u8; 12])
    .execute(tx.conn())
    .await;
    assert_eq!(timelex_db::testing::code(&restore.unwrap_err()), "23514");
}

#[tokio::test]
async fn every_vault_table_has_the_standard_tenant_policy() {
    let Some(env) = env().await else { return };
    let unprotected: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM platform.tables_without_tenant_rls WHERE schema_name = 'vault'",
    )
    .fetch_all(&env.db.admin)
    .await
    .unwrap();
    assert!(unprotected.is_empty(), "unprotected: {unprotected:?}");
}

#[tokio::test]
async fn two_firms_have_independent_chains_that_both_start_at_one() {
    let Some(env) = env().await else { return };
    let (a, b) = (
        TenantId::from_uuid(Uuid::new_v4()),
        TenantId::from_uuid(Uuid::new_v4()),
    );
    for tenant in [a, b] {
        env.store
            .put(
                settings(tenant),
                SecretKind::AnthropicApiKey,
                request(ANTHROPIC_V1),
            )
            .await
            .unwrap();
    }
    for tenant in [a, b] {
        let first: i64 = sqlx::query_scalar("SELECT min(seq) FROM vault.secret_audit WHERE tenant_id = $1")
            .bind(tenant.as_uuid())
            .fetch_one(&env.db.admin)
            .await
            .unwrap();
        assert_eq!(first, 1);
        assert_chain_verifies(&env.db, tenant).await;
    }
}

#[test]
fn the_test_value_is_long_enough_to_be_a_secret() {
    // Guards the constants above: `prepare` requires 8..=4096 printable characters.
    assert!((8..=4096).contains(&ANTHROPIC_V1.len()));
    assert!((8..=4096).contains(&ANTHROPIC_V2.len()));
}

/// A naive subslice search, so the test does not need a dependency.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------- events for other services (P1-06) ----------

#[tokio::test]
async fn every_write_appends_one_secret_event_without_the_value() {
    let Some(env) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let actor = settings(tenant);
    let PutOutcome::Created(_) = env
        .store
        .put(actor.clone(), SecretKind::AnthropicApiKey, request(ANTHROPIC_V1))
        .await
        .unwrap()
    else {
        panic!("creates");
    };
    env.clock.advance(Duration::seconds(1));
    let PutOutcome::Replaced(meta) = env
        .store
        .put(actor.clone(), SecretKind::AnthropicApiKey, request(ANTHROPIC_V2))
        .await
        .unwrap()
    else {
        panic!("replaces");
    };
    env.clock.advance(Duration::seconds(1));
    env.store
        .record_test(actor.clone(), meta.id, TestOutcome::Passed)
        .await
        .unwrap();
    env.clock.advance(Duration::seconds(1));
    env.store
        .rotate(actor.clone(), meta.id, ANTHROPIC_V1.to_owned())
        .await
        .unwrap();
    env.clock.advance(Duration::seconds(1));
    env.store.revoke(actor.clone(), meta.id).await.unwrap();
    // Revoking twice logs once, so it must publish once.
    env.store.revoke(actor.clone(), meta.id).await.unwrap();

    let rows =
        sqlx::query("SELECT event_type, payload FROM platform.outbox WHERE tenant_id = $1 ORDER BY seq")
            .bind(tenant.as_uuid())
            .fetch_all(&env.db.admin)
            .await
            .unwrap();
    let types: Vec<String> = rows.iter().map(|row| row.get("event_type")).collect();
    assert_eq!(
        types,
        [
            "secret.created",
            "secret.updated",
            "secret.tested",
            "secret.rotated",
            "secret.revoked"
        ]
    );

    let mut ids = std::collections::HashSet::new();
    for row in &rows {
        let event_type: String = row.get("event_type");
        let payload: serde_json::Value = row.get("payload");
        let text = payload.to_string();
        assert!(!text.contains(ANTHROPIC_V1), "the old value is not in the event");
        assert!(!text.contains(ANTHROPIC_V2), "the new value is not in the event");
        assert!(
            !text.contains("sk-ant-api03"),
            "no part of a value is in the event"
        );
        assert_eq!(payload["type"], event_type, "the column and envelope agree");
        assert_eq!(payload["tenantId"], format!("tn_{}", tenant.to_hex32()));
        assert_eq!(payload["data"]["secretId"], meta.id.as_uuid().to_string());
        assert_eq!(payload["data"]["kind"], "anthropic_api_key");
        assert_eq!(payload["data"]["label"], "Firm Claude key");
        let last_four = match event_type.as_str() {
            "secret.created" => "1111",
            "secret.updated" | "secret.tested" => "9999",
            _ => "1111",
        };
        assert_eq!(payload["data"]["lastFour"], last_four);
        let id = payload["id"].as_str().unwrap();
        assert!(id.starts_with("ev_") && id.len() == 35, "{id}");
        assert!(ids.insert(id.to_owned()), "event ids are unique");
    }

    let tested = rows
        .iter()
        .find(|row| row.get::<String, _>("event_type") == "secret.tested")
        .unwrap();
    let payload: serde_json::Value = tested.get("payload");
    assert_eq!(payload["data"]["result"], "passed");
    assert_eq!(payload["source"], "vault");
    assert_chain_verifies(&env.db, tenant).await;
}
