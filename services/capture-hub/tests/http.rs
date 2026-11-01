// Written by Kone Tshivhinda & Claude
//! P2-02: `POST /v1/events`. The firm and user come from the verified token; the body names them
//! only to be checked against it.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use time::{Duration, OffsetDateTime};
use timelex_auth::{generate_keypair, Caller, Principal, Signer, Verifier};
use timelex_capture_service::{http, CaptureStore, Clock, MIGRATIONS};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use timelex_types as types;
use timelex_types::ActivityEvent;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct TestClock(Arc<Mutex<OffsetDateTime>>);

impl TestClock {
    fn new() -> Self {
        let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
        Self(Arc::new(Mutex::new(now)))
    }

    fn clock(&self) -> Clock {
        let t = self.0.clone();
        Arc::new(move || *t.lock().unwrap())
    }
}

struct Env {
    db: TestDb,
    app: axum::Router,
    signer: Signer,
    tenant: TenantId,
    user: UserId,
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
    let store = CaptureStore::with_clock(tenants, clock.clock());
    let keys = generate_keypair().unwrap();
    let signer = Signer::from_base64("test-key", &keys.private_pkcs8_base64).unwrap();
    let verifier = Verifier::from_config("capture", &format!("test-key={}", keys.public_base64)).unwrap();
    let app = http::router(store, verifier);
    Some(Env {
        db,
        app,
        signer,
        tenant: TenantId::from_uuid(Uuid::new_v4()),
        user: UserId::from_uuid(Uuid::new_v4()),
        clock,
    })
}

fn token_for(env: &Env, caller: &Caller) -> String {
    env.signer
        .sign_at("capture", caller, *env.clock.0.lock().unwrap())
        .unwrap()
}

fn principal(env: &Env, permissions: &[Permission]) -> Principal {
    Principal {
        tenant: env.tenant,
        user: env.user,
        key: ApiKeyId::from_uuid(Uuid::new_v4()),
        permissions: permissions.iter().copied().collect(),
    }
}

fn event_for(env: &Env, tenant: TenantId, user: UserId, source_id: &str) -> ActivityEvent {
    let started = *env.clock.0.lock().unwrap();
    let mut event = ActivityEvent {
        schema_version: types::ACTIVITY_EVENT_SCHEMA_VERSION.to_owned(),
        id: String::new(),
        tenant_id: format!("tn_{}", tenant.to_hex32()),
        source: types::Source {
            system: types::SourceSystem::Manual,
            source_id: source_id.to_owned(),
        },
        kind: types::ActivityKind::ManualEntry,
        actor: types::Actor {
            user_id: format!("us_{}", user.as_uuid().simple()),
        },
        started_at: started,
        ended_at: started + Duration::minutes(5),
        captured_at: started + Duration::minutes(6),
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

fn post(body: &ActivityEvent, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

async fn count_events(db: &TestDb, tenant: TenantId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM capture.activity_events WHERE tenant_id = $1")
        .bind(tenant.as_uuid())
        .fetch_one(&db.admin)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_new_event_is_accepted_with_201() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["id"], event.id);
    assert_eq!(body["created"], true);
    assert_eq!(count_events(&env.db, env.tenant).await, 1);
}

#[tokio::test]
async fn a_duplicate_delivery_is_accepted_with_200_and_created_false() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (first, _) = send(env.app.clone(), post(&event, &token)).await;
    let (second, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(first, StatusCode::CREATED);
    assert_eq!(second, StatusCode::OK);
    assert_eq!(body["created"], false);
    assert_eq!(count_events(&env.db, env.tenant).await, 1);
}

#[tokio::test]
async fn a_missing_capture_write_is_forbidden() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let token = token_for(&env, &Caller::Principal(principal(&env, &[Permission::TimeRead])));

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(count_events(&env.db, env.tenant).await, 0);
}

#[tokio::test]
async fn an_unauthenticated_request_is_401() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let request = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&event).unwrap()))
        .unwrap();
    let (status, _) = send(env.app.clone(), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_service_token_cannot_submit_an_event() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let token = token_for(&env, &Caller::Service);
    let (status, _) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn another_firms_event_is_rejected() {
    let Some(env) = env().await else { return };
    let other = TenantId::from_uuid(Uuid::new_v4());
    let event = event_for(&env, other, env.user, "synthetic-manual-a");
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(count_events(&env.db, env.tenant).await, 0);
    assert_eq!(count_events(&env.db, other).await, 0);
}

#[tokio::test]
async fn another_users_event_is_rejected() {
    let Some(env) = env().await else { return };
    let other = UserId::from_uuid(Uuid::new_v4());
    let event = event_for(&env, env.tenant, other, "synthetic-manual-a");
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(count_events(&env.db, env.tenant).await, 0);
}

#[tokio::test]
async fn the_tenant_and_actor_are_stored_in_their_canonical_spelling() {
    let Some(env) = env().await else { return };
    let mut event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    // The caller may use the hyphenated UUIDs; storage has one spelling.
    event.tenant_id = format!("tn_{}", env.tenant.as_uuid());
    event.actor.user_id = format!("us_{}", env.user.as_uuid());
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let stored: (String, String, String) = sqlx::query_as(
        "SELECT actor_id, event->>'tenantId', event->'actor'->>'userId' \
         FROM capture.activity_events WHERE tenant_id = $1",
    )
    .bind(env.tenant.as_uuid())
    .fetch_one(&env.db.admin)
    .await
    .unwrap();
    assert_eq!(stored.0, format!("us_{}", env.user.as_uuid().simple()));
    assert_eq!(stored.1, format!("tn_{}", env.tenant.to_hex32()));
    assert_eq!(stored.2, format!("us_{}", env.user.as_uuid().simple()));
}

#[tokio::test]
async fn an_id_that_does_not_match_the_event_is_rejected() {
    let Some(env) = env().await else { return };
    let mut event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    event.id = "ae_00000000000000000000000000000000".into();
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(count_events(&env.db, env.tenant).await, 0);
}

#[tokio::test]
async fn an_unsupported_major_is_rejected() {
    let Some(env) = env().await else { return };
    let mut event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    event.schema_version = "2.0".into();
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let (status, body) = send(env.app.clone(), post(&event, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn a_newer_minor_is_accepted() {
    let Some(env) = env().await else { return };
    let event = event_for(&env, env.tenant, env.user, "synthetic-manual-a");
    let mut value = serde_json::to_value(&event).unwrap();
    value["schemaVersion"] = serde_json::Value::String("1.1".into());
    value["capturedBy"] = serde_json::Value::String("desktop-agent".into());
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );

    let request = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap();
    let (status, body) = send(env.app.clone(), request).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(count_events(&env.db, env.tenant).await, 1);
}

#[tokio::test]
async fn a_malformed_body_is_400_not_500() {
    let Some(env) = env().await else { return };
    let token = token_for(
        &env,
        &Caller::Principal(principal(&env, &[Permission::CaptureWrite])),
    );
    let request = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from("{\"schemaVersion\":"))
        .unwrap();
    let (status, _) = send(env.app.clone(), request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_answers() {
    let Some(env) = env().await else { return };
    let request = Request::builder().uri("/health").body(Body::empty()).unwrap();
    let (status, body) = send(env.app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}
