// Written by Kone Tshivhinda & Claude
//! P2-02 acceptance: duplicate delivery yields one event, against a real Postgres with the
//! production roles and row-level security.
//!
//! The connector runner is tested separately; these tests stop at the store, which is where the
//! idempotency guarantee lives.

use std::sync::{Arc, Mutex};

use sqlx::Row;
use time::{Duration, OffsetDateTime};
use timelex_capture_service::{
    Actor, CaptureStore, Clock, ExclusionKind, IngestOutcome, PageOutcome, StoreError, MIGRATIONS,
};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_db::TenantPool;
use timelex_identity::{Permission, TenantId, UserId};
use timelex_types as types;
use timelex_types::ActivityEvent;
use uuid::Uuid;

#[derive(Clone)]
struct TestClock(Arc<Mutex<OffsetDateTime>>);

impl TestClock {
    fn new() -> Self {
        // Whole microseconds, which is Postgres's precision.
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
}

struct Env {
    db: TestDb,
    store: CaptureStore,
    tenants: TenantPool,
    clock: TestClock,
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "capture",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let clock = TestClock::new();
    let store = CaptureStore::with_clock(tenants.clone(), clock.clock());
    Some(Env {
        db,
        store,
        tenants,
        clock,
    })
}

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4())
}

fn user() -> UserId {
    UserId::from_uuid(Uuid::new_v4())
}

fn actor(tenant: TenantId, user: UserId, permissions: &[Permission]) -> Actor {
    Actor {
        tenant,
        user,
        permissions: permissions.iter().copied().collect(),
    }
}

fn writer(tenant: TenantId, user: UserId) -> Actor {
    actor(tenant, user, &[Permission::CaptureWrite])
}

fn privacy_admin(tenant: TenantId, user: UserId) -> Actor {
    actor(tenant, user, &[Permission::SettingsManage])
}

fn event(tenant: TenantId, user: UserId, source_id: &str, started: OffsetDateTime) -> ActivityEvent {
    let mut event = ActivityEvent {
        schema_version: types::ACTIVITY_EVENT_SCHEMA_VERSION.to_owned(),
        id: String::new(),
        tenant_id: format!("tn_{}", tenant.to_hex32()),
        source: types::Source {
            system: types::SourceSystem::M365Mail,
            source_id: source_id.to_owned(),
        },
        kind: types::ActivityKind::EmailSent,
        actor: types::Actor {
            user_id: format!("us_{}", user.as_uuid().simple()),
        },
        started_at: started,
        ended_at: started + Duration::minutes(8),
        captured_at: started + Duration::minutes(9),
        subject: None,
        content_ref: None,
        privacy: types::Privacy {
            content_captured: false,
            redaction_applied: false,
        },
    };
    event.id = event.expected_id();
    event
}

fn event_with_domain(
    tenant: TenantId,
    user: UserId,
    source_id: &str,
    started: OffsetDateTime,
    domain: &str,
) -> ActivityEvent {
    let mut event = event(tenant, user, source_id, started);
    event.kind = types::ActivityKind::EmailReceived;
    event.subject = Some(types::Subject {
        participants: vec![types::Participant {
            role: types::ParticipantRole::From,
            domain: domain.to_owned(),
            address: None,
        }],
        ..Default::default()
    });
    event.id = event.expected_id();
    event
}

async fn count_events(tenants: &TenantPool, tenant: TenantId) -> i64 {
    let mut tx = tenants.begin(tenant).await.unwrap();
    let count = sqlx::query_scalar("SELECT count(*) FROM capture.activity_events")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    count
}

#[tokio::test]
async fn acceptance_duplicate_delivery_yields_one_event() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();
    let signal = event(tenant, user, "AAMkAGI2synthetic-message-id", at);

    let first = env
        .store
        .ingest(writer(tenant, user), signal.clone())
        .await
        .unwrap();
    assert_eq!(first.outcome, IngestOutcome::Created);
    assert_eq!(first.id, signal.id);

    // The same signal arrives again: same event, no second row, no second publication.
    let second = env
        .store
        .ingest(writer(tenant, user), signal.clone())
        .await
        .unwrap();
    assert_eq!(second.outcome, IngestOutcome::Existing);
    assert_eq!(second.id, signal.id);

    assert_eq!(count_events(&env.tenants, tenant).await, 1);
    let published: i64 = sqlx::query_scalar("SELECT count(*) FROM platform.outbox WHERE tenant_id = $1")
        .bind(tenant.as_uuid())
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(published, 1, "a duplicate publishes nothing");
}

#[tokio::test]
async fn a_new_event_is_stored_with_its_envelope_and_published_once() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();
    let signal = event(tenant, user, "AAMkAGI2synthetic-message-id", at);

    env.store
        .ingest(writer(tenant, user), signal.clone())
        .await
        .unwrap();

    let mut tx = env.tenants.begin(tenant).await.unwrap();
    let row = sqlx::query(
        "SELECT id, source_system, source_id, actor_id, kind, started_at, ended_at, captured_at, \
         recorded_at, event FROM capture.activity_events WHERE id = $1",
    )
    .bind(&signal.id)
    .fetch_one(tx.conn())
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(row.get::<String, _>("id"), signal.id);
    assert_eq!(row.get::<String, _>("source_system"), "m365_mail");
    assert_eq!(row.get::<String, _>("source_id"), "AAMkAGI2synthetic-message-id");
    assert_eq!(
        row.get::<String, _>("actor_id"),
        format!("us_{}", user.as_uuid().simple())
    );
    assert_eq!(row.get::<String, _>("kind"), "email_sent");
    assert_eq!(row.get::<OffsetDateTime, _>("started_at"), signal.started_at);
    assert_eq!(row.get::<OffsetDateTime, _>("ended_at"), signal.ended_at);
    assert_eq!(row.get::<OffsetDateTime, _>("recorded_at"), env.clock.now());
    let stored: serde_json::Value = row.get("event");
    assert_eq!(stored, serde_json::to_value(&signal).unwrap());

    let outbox =
        sqlx::query("SELECT event_type, payload, created_at FROM platform.outbox WHERE tenant_id = $1")
            .bind(tenant.as_uuid())
            .fetch_one(&env.db.admin)
            .await
            .unwrap();
    assert_eq!(outbox.get::<String, _>("event_type"), "activity.recorded");
    let payload: serde_json::Value = outbox.get("payload");
    assert_eq!(payload["id"], signal.id);
    assert_eq!(payload["tenantId"], format!("tn_{}", tenant.to_hex32()));
}

#[tokio::test]
async fn a_different_signal_is_a_different_event() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();

    let first = event(tenant, user, "synthetic-message-a", at);
    let second = event(tenant, user, "synthetic-message-b", at);
    let third = event(tenant, user, "synthetic-message-a", at + Duration::minutes(1));

    for e in [&first, &second, &third] {
        assert_eq!(
            env.store
                .ingest(writer(tenant, user), e.clone())
                .await
                .unwrap()
                .outcome,
            IngestOutcome::Created
        );
    }
    assert_eq!(count_events(&env.tenants, tenant).await, 3);
}

#[tokio::test]
async fn another_firm_cannot_see_the_event() {
    let Some(env) = env().await else { return };
    let tenant_a = tenant();
    let tenant_b = tenant();
    let user = user();
    let signal = event(tenant_a, user, "synthetic-message-a", env.clock.now());

    env.store
        .ingest(writer(tenant_a, user), signal.clone())
        .await
        .unwrap();

    let mut tx = env.tenants.begin(tenant_b).await.unwrap();
    let visible: Option<String> = sqlx::query_scalar("SELECT id FROM capture.activity_events WHERE id = $1")
        .bind(&signal.id)
        .fetch_optional(tx.conn())
        .await
        .unwrap();
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM capture.activity_events")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(visible, None);
    assert_eq!(total, 0);
}

#[tokio::test]
async fn the_service_role_cannot_change_or_delete_an_event() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    env.store
        .ingest(
            writer(tenant, user),
            event(tenant, user, "synthetic-message-a", env.clock.now()),
        )
        .await
        .unwrap();

    let (service, _) = env.db.service_pool().await;
    let update = sqlx::query("UPDATE capture.activity_events SET kind = 'meeting'")
        .execute(&service)
        .await
        .unwrap_err();
    assert_eq!(timelex_db::testing::code(&update), "42501", "no UPDATE grant");
    let delete = sqlx::query("DELETE FROM capture.activity_events")
        .execute(&service)
        .await
        .unwrap_err();
    assert_eq!(timelex_db::testing::code(&delete), "42501", "no DELETE grant");
}

#[tokio::test]
async fn an_id_that_does_not_match_its_content_is_rejected() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let mut signal = event(tenant, user, "synthetic-message-a", env.clock.now());
    signal.id = "ae_00000000000000000000000000000000".into();

    let error = env.store.ingest(writer(tenant, user), signal).await.unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
    assert_eq!(count_events(&env.tenants, tenant).await, 0);
}

#[tokio::test]
async fn an_unsupported_version_is_rejected() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let mut signal = event(tenant, user, "synthetic-message-a", env.clock.now());
    signal.schema_version = "2.0".into();

    let error = env.store.ingest(writer(tenant, user), signal).await.unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[tokio::test]
async fn an_event_that_ends_before_it_starts_is_rejected() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let mut signal = event(tenant, user, "synthetic-message-a", env.clock.now());
    signal.ended_at = signal.started_at - Duration::minutes(1);

    let error = env.store.ingest(writer(tenant, user), signal).await.unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[tokio::test]
async fn ingesting_needs_capture_write() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let signal = event(tenant, user, "synthetic-message-a", env.clock.now());

    let error = env
        .store
        .ingest(actor(tenant, user, &[Permission::TimeRead]), signal)
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Forbidden));
    assert_eq!(count_events(&env.tenants, tenant).await, 0);
}

#[tokio::test]
async fn a_page_stores_every_event_and_moves_the_cursor() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();
    let page = [
        event(tenant, user, "synthetic-message-a", at),
        event(tenant, user, "synthetic-message-b", at),
    ];

    let outcome = env
        .store
        .record_page(tenant, "m365_mail", &page, Some("page-2"))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        PageOutcome {
            created: 2,
            existing: 0,
            excluded: 0
        }
    );
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("page-2".into())
    );

    // Re-delivery of the whole page: nothing new, cursor still moves to the page's next marker.
    let again = env
        .store
        .record_page(tenant, "m365_mail", &page, Some("page-3"))
        .await
        .unwrap();
    assert_eq!(
        again,
        PageOutcome {
            created: 0,
            existing: 2,
            excluded: 0
        }
    );
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("page-3".into())
    );
    assert_eq!(count_events(&env.tenants, tenant).await, 2);
}

#[tokio::test]
async fn a_bad_event_fails_the_whole_page_and_leaves_the_cursor_alone() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();
    let good = event(tenant, user, "synthetic-message-a", at);
    env.store
        .record_page(tenant, "m365_mail", std::slice::from_ref(&good), Some("page-2"))
        .await
        .unwrap();

    let good_2 = event(tenant, user, "synthetic-message-b", at);
    let mut bad = event(tenant, user, "synthetic-message-c", at);
    bad.id = "ae_00000000000000000000000000000000".into();
    let error = env
        .store
        .record_page(tenant, "m365_mail", &[good_2, bad], Some("page-3"))
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));

    assert_eq!(count_events(&env.tenants, tenant).await, 1, "page is atomic");
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("page-2".into()),
        "the cursor did not move past the page that failed"
    );
}

#[tokio::test]
async fn a_cursor_is_per_connector_and_per_firm() {
    let Some(env) = env().await else { return };
    let tenant_a = tenant();
    let tenant_b = tenant();
    let user = user();
    let at = env.clock.now();

    env.store
        .record_page(
            tenant_a,
            "m365_mail",
            &[event(tenant_a, user, "synthetic-message-a", at)],
            Some("a-2"),
        )
        .await
        .unwrap();
    env.store
        .record_page(
            tenant_a,
            "google_mail",
            &[event(tenant_a, user, "synthetic-message-g", at)],
            Some("g-9"),
        )
        .await
        .unwrap();

    assert_eq!(
        env.store.cursor(tenant_a, "m365_mail").await.unwrap(),
        Some("a-2".into())
    );
    assert_eq!(
        env.store.cursor(tenant_a, "google_mail").await.unwrap(),
        Some("g-9".into())
    );
    assert_eq!(env.store.cursor(tenant_b, "m365_mail").await.unwrap(), None);
    assert_eq!(env.store.cursor(tenant_a, "desktop_agent").await.unwrap(), None);
}

#[tokio::test]
async fn a_bad_connector_name_or_cursor_is_rejected() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    for name in ["", "M365", "9mail", "a-b", &"x".repeat(65)] {
        let error = env.store.cursor(tenant, name).await.unwrap_err();
        assert!(matches!(error, StoreError::Invalid(_)), "{name:?}");
    }
    let error = env
        .store
        .record_page(tenant, "m365_mail", &[], Some(""))
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
    let long = "x".repeat(4097);
    let error = env
        .store
        .record_page(tenant, "m365_mail", &[], Some(&long))
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

async fn count_outbox(db: &TestDb, tenant: TenantId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM platform.outbox WHERE tenant_id = $1")
        .bind(tenant.as_uuid())
        .fetch_one(&db.admin)
        .await
        .unwrap()
}

async fn count_exclusions(tenants: &TenantPool, tenant: TenantId) -> i64 {
    let mut tx = tenants.begin(tenant).await.unwrap();
    let count = sqlx::query_scalar("SELECT count(*) FROM capture.exclusions")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    count
}

#[tokio::test]
async fn an_excluded_domain_is_never_stored_and_nothing_is_published() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let (exclusion, created) = env
        .store
        .add_exclusion(
            privacy_admin(tenant, user),
            ExclusionKind::Domain,
            " Example.COM. ",
            Some(" client asked "),
        )
        .await
        .unwrap();
    assert!(created);
    assert_eq!(exclusion.value, "example.com");
    assert_eq!(exclusion.reason.as_deref(), Some("client asked"));

    let signal = event_with_domain(
        tenant,
        user,
        "synthetic-message-a",
        env.clock.now(),
        "mail.example.com",
    );
    let outcome = env.store.ingest(writer(tenant, user), signal).await.unwrap();
    assert_eq!(outcome.outcome, IngestOutcome::Excluded);
    assert_eq!(count_events(&env.tenants, tenant).await, 0);
    assert_eq!(count_outbox(&env.db, tenant).await, 0);
}

#[tokio::test]
async fn a_page_skips_excluded_events_and_still_moves_the_cursor() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();
    env.store
        .add_exclusion(
            privacy_admin(tenant, user),
            ExclusionKind::Domain,
            "example.com",
            None,
        )
        .await
        .unwrap();

    let page = [
        event_with_domain(tenant, user, "synthetic-message-a", at, "mail.example.com"),
        event(tenant, user, "synthetic-message-b", at),
    ];
    let outcome = env
        .store
        .record_page(tenant, "m365_mail", &page, Some("page-2"))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        PageOutcome {
            created: 1,
            existing: 0,
            excluded: 1
        }
    );
    assert_eq!(count_events(&env.tenants, tenant).await, 1);
    assert_eq!(count_outbox(&env.db, tenant).await, 1);
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("page-2".into()),
        "one excluded signal must not stall the connector"
    );
}

#[tokio::test]
async fn exclusions_are_per_firm() {
    let Some(env) = env().await else { return };
    let tenant_a = tenant();
    let tenant_b = tenant();
    let user = user();
    let at = env.clock.now();
    env.store
        .add_exclusion(
            privacy_admin(tenant_a, user),
            ExclusionKind::Domain,
            "example.com",
            None,
        )
        .await
        .unwrap();

    let signal = event_with_domain(tenant_b, user, "synthetic-message-b", at, "mail.example.com");
    let outcome = env.store.ingest(writer(tenant_b, user), signal).await.unwrap();
    assert_eq!(
        outcome.outcome,
        IngestOutcome::Created,
        "another firm's exclusion does not apply"
    );
    assert_eq!(count_events(&env.tenants, tenant_b).await, 1);
    assert!(env
        .store
        .list_exclusions(privacy_admin(tenant_b, user))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn managing_exclusions_needs_settings_manage() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();

    let add = env
        .store
        .add_exclusion(writer(tenant, user), ExclusionKind::Domain, "example.com", None)
        .await
        .unwrap_err();
    assert!(matches!(add, StoreError::Forbidden));
    let list = env.store.list_exclusions(writer(tenant, user)).await.unwrap_err();
    assert!(matches!(list, StoreError::Forbidden));
    let remove = env
        .store
        .remove_exclusion(writer(tenant, user), ExclusionKind::Domain, "example.com")
        .await
        .unwrap_err();
    assert!(matches!(remove, StoreError::Forbidden));
    assert_eq!(count_exclusions(&env.tenants, tenant).await, 0);
}

#[tokio::test]
async fn adding_an_exclusion_twice_keeps_one_row_and_the_first_reason() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let admin = privacy_admin(tenant, user);

    let (_, first) = env
        .store
        .add_exclusion(admin.clone(), ExclusionKind::Domain, "example.com", Some("first"))
        .await
        .unwrap();
    let (exclusion, second) = env
        .store
        .add_exclusion(admin, ExclusionKind::Domain, "EXAMPLE.COM", Some("second"))
        .await
        .unwrap();
    assert!(first);
    assert!(!second, "a repeat changes nothing");
    assert_eq!(exclusion.reason.as_deref(), Some("first"));
    assert_eq!(count_exclusions(&env.tenants, tenant).await, 1);

    let listed = env
        .store
        .list_exclusions(privacy_admin(tenant, user))
        .await
        .unwrap();
    assert_eq!(listed, vec![exclusion]);
}

#[tokio::test]
async fn removing_an_exclusion_stops_filtering() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let admin = privacy_admin(tenant, user);
    env.store
        .add_exclusion(admin.clone(), ExclusionKind::Domain, "example.com", None)
        .await
        .unwrap();

    assert!(env
        .store
        .remove_exclusion(admin.clone(), ExclusionKind::Domain, " Example.com ")
        .await
        .unwrap());
    assert!(!env
        .store
        .remove_exclusion(admin, ExclusionKind::Domain, "example.com")
        .await
        .unwrap());
    assert_eq!(count_exclusions(&env.tenants, tenant).await, 0);

    let signal = event_with_domain(
        tenant,
        user,
        "synthetic-message-a",
        env.clock.now(),
        "mail.example.com",
    );
    let outcome = env.store.ingest(writer(tenant, user), signal).await.unwrap();
    assert_eq!(outcome.outcome, IngestOutcome::Created);
}

#[tokio::test]
async fn a_client_exclusion_is_recorded_but_does_not_filter_capture_yet() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let (exclusion, created) = env
        .store
        .add_exclusion(
            privacy_admin(tenant, user),
            ExclusionKind::Client,
            "CL-2026-0042",
            None,
        )
        .await
        .unwrap();
    assert!(created);
    assert_eq!(exclusion.kind, ExclusionKind::Client);

    // A raw signal names no client, so client exclusions take effect at attribution (P3).
    let signal = event_with_domain(
        tenant,
        user,
        "synthetic-message-a",
        env.clock.now(),
        "example.com",
    );
    let outcome = env.store.ingest(writer(tenant, user), signal).await.unwrap();
    assert_eq!(outcome.outcome, IngestOutcome::Created);
}

#[tokio::test]
async fn an_invalid_exclusion_is_rejected_and_writes_nothing() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let admin = privacy_admin(tenant, user);

    assert!(matches!(
        ExclusionKind::parse("phone"),
        Err(StoreError::Invalid(_))
    ));
    let domain = env
        .store
        .add_exclusion(admin.clone(), ExclusionKind::Domain, "not a domain", None)
        .await
        .unwrap_err();
    assert!(matches!(domain, StoreError::Invalid(_)));
    let reason = env
        .store
        .add_exclusion(
            admin,
            ExclusionKind::Domain,
            "example.com",
            Some(&"x".repeat(501)),
        )
        .await
        .unwrap_err();
    assert!(matches!(reason, StoreError::Invalid(_)));
    assert_eq!(count_exclusions(&env.tenants, tenant).await, 0);
}

#[tokio::test]
async fn content_is_stored_only_when_it_was_captured_and_redacted() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let user = user();
    let at = env.clock.now();

    let mut unredacted = event(tenant, user, "synthetic-message-a", at);
    unredacted.privacy.content_captured = true;
    unredacted.content_ref = Some("s3://content/raw".into());
    let error = env
        .store
        .ingest(writer(tenant, user), unredacted)
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));

    let mut orphan = event(tenant, user, "synthetic-message-b", at);
    orphan.privacy.content_captured = false;
    orphan.content_ref = Some("s3://content/raw".into());
    let error = env.store.ingest(writer(tenant, user), orphan).await.unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));

    let mut missing = event(tenant, user, "synthetic-message-c", at);
    missing.privacy.content_captured = true;
    missing.privacy.redaction_applied = true;
    let error = env.store.ingest(writer(tenant, user), missing).await.unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
    assert_eq!(count_events(&env.tenants, tenant).await, 0);

    let mut redacted = event(tenant, user, "synthetic-message-d", at);
    redacted.privacy.content_captured = true;
    redacted.privacy.redaction_applied = true;
    redacted.content_ref = Some("s3://content/redacted".into());
    let outcome = env.store.ingest(writer(tenant, user), redacted).await.unwrap();
    assert_eq!(outcome.outcome, IngestOutcome::Created);
    assert_eq!(count_events(&env.tenants, tenant).await, 1);
}

#[tokio::test]
async fn an_empty_page_still_records_the_connector_as_caught_up() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let outcome = env
        .store
        .record_page(tenant, "m365_mail", &[], Some("up-to-here"))
        .await
        .unwrap();
    assert_eq!(outcome, PageOutcome::default());
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("up-to-here".into())
    );
}
