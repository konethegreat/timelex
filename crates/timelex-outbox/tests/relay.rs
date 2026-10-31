// Written by Kone Tshivhinda & Claude
//! P1-06 acceptance: an event written in the producer's transaction is relayed at least once, and
//! an idempotent consumer sees exactly one effect even after a redelivery.
//!
//! These tests use a real Postgres with the production bootstrap, so the queue, its grants and the
//! relay's claim query are exercised exactly as deployed.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_identity::TenantId;
use timelex_outbox::{append, Event, EventId, Publisher, QueuedEvent, Relay};
use uuid::Uuid;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./tests/migrations");

async fn env() -> Option<(TestDb, PgPool)> {
    let db = setup_db(&ServiceUnderTest {
        service: "identity",
        migrations: &MIGRATIONS,
    })
    .await?;
    let pool = db.as_role("svc_identity").await;
    Some((db, pool))
}

fn event(tenant: TenantId) -> Event {
    Event {
        id: EventId::new(),
        tenant,
        event_type: "secret.created".into(),
        occurred_at: OffsetDateTime::now_utc(),
        payload: json!({
            "schemaVersion": "1.0",
            "type": "secret.created",
            "tenantId": format!("tn_{}", tenant.to_hex32()),
        }),
    }
}

async fn write(pool: &PgPool, event: &Event) {
    let mut tx = pool.begin().await.unwrap();
    append(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
}

async fn pending(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM platform.outbox WHERE published_at IS NULL")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A consumer that records deliveries and applies each event id once, as a real consumer must.
#[derive(Clone, Default)]
struct Sink {
    delivered: Arc<Mutex<Vec<Uuid>>>,
    seen: Arc<Mutex<HashSet<Uuid>>>,
    effects: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
}

impl Sink {
    fn effects(&self) -> usize {
        self.effects.load(Ordering::SeqCst)
    }
    fn delivered(&self) -> Vec<Uuid> {
        self.delivered.lock().unwrap().clone()
    }
}

#[async_trait]
impl Publisher for Sink {
    async fn publish(&self, event: &QueuedEvent) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("synthetic provider failure".into());
        }
        self.delivered.lock().unwrap().push(event.id);
        let mut seen = self.seen.lock().unwrap();
        if seen.insert(event.id) {
            self.effects.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[tokio::test]
async fn an_event_joins_the_writers_transaction() {
    let Some((db, pool)) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let e = event(tenant);

    let mut tx = pool.begin().await.unwrap();
    append(&mut tx, &e).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(pending(&pool).await, 0, "a rolled-back write publishes nothing");

    write(&pool, &e).await;
    assert_eq!(pending(&pool).await, 1);

    let (id, tenant_id, event_type, payload): (Uuid, Uuid, String, serde_json::Value) =
        sqlx::query_as("SELECT id, tenant_id, event_type, payload FROM platform.outbox")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(id, e.id.as_uuid());
    assert_eq!(tenant_id, tenant.as_uuid());
    assert_eq!(event_type, "secret.created");
    assert_eq!(payload, e.payload);
    db.cleanup().await;
}

#[tokio::test]
async fn each_claimed_event_is_published_once_and_the_order_is_kept() {
    let Some((db, pool)) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let a = event(tenant);
    let b = event(tenant);
    write(&pool, &a).await;
    write(&pool, &b).await;

    let sink = Sink::default();
    let relay_a = Relay::new(pool.clone(), sink.clone());
    let relay_b = Relay::new(pool.clone(), sink.clone());

    assert_eq!(relay_a.poll_once(1).await.unwrap(), 1);
    assert_eq!(
        relay_b.poll_once(1).await.unwrap(),
        1,
        "a second relay takes the next row"
    );
    assert_eq!(relay_a.poll_once(1).await.unwrap(), 0);

    assert_eq!(sink.delivered(), vec![a.id.as_uuid(), b.id.as_uuid()]);
    assert_eq!(pending(&pool).await, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn a_failed_publish_backs_off_and_a_later_poll_retries_it() {
    let Some((db, pool)) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let e = event(tenant);
    write(&pool, &e).await;

    let sink = Sink::default();
    sink.fail.store(true, Ordering::SeqCst);
    let relay = Relay::new(pool.clone(), sink.clone());
    assert_eq!(relay.poll_once(10).await.unwrap(), 1);

    let (published, attempts, last_error, next_attempt): (
        Option<OffsetDateTime>,
        i32,
        Option<String>,
        OffsetDateTime,
    ) = sqlx::query_as(
        "SELECT published_at, attempts, last_error, next_attempt_at \
         FROM platform.outbox WHERE id = $1",
    )
    .bind(e.id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(published.is_none());
    assert_eq!(attempts, 1);
    assert!(last_error.is_some(), "the failure is recorded");
    assert!(next_attempt > OffsetDateTime::now_utc(), "the retry waits");
    assert_eq!(relay.poll_once(10).await.unwrap(), 0, "nothing is due yet");

    sqlx::query("UPDATE platform.outbox SET next_attempt_at = now() WHERE id = $1")
        .bind(e.id.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    sink.fail.store(false, Ordering::SeqCst);
    assert_eq!(relay.poll_once(10).await.unwrap(), 1);

    let (published, attempts, last_error): (Option<OffsetDateTime>, i32, Option<String>) =
        sqlx::query_as("SELECT published_at, attempts, last_error FROM platform.outbox WHERE id = $1")
            .bind(e.id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(published.is_some());
    assert_eq!(attempts, 2);
    assert!(last_error.is_none(), "a later success clears the old error");
    db.cleanup().await;
}

#[tokio::test]
async fn a_redelivery_after_a_crash_has_one_effect_for_an_idempotent_consumer() {
    let Some((db, pool)) = env().await else { return };
    let tenant = TenantId::from_uuid(Uuid::new_v4());
    let e = event(tenant);
    write(&pool, &e).await;

    let sink = Sink::default();
    let relay = Relay::new(pool.clone(), sink.clone());
    assert_eq!(relay.poll_once(10).await.unwrap(), 1);

    // The consumer applied it, then the relay died before recording that it had published.
    sqlx::query("UPDATE platform.outbox SET published_at = NULL, next_attempt_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(relay.poll_once(10).await.unwrap(), 1);

    assert_eq!(sink.delivered().len(), 2, "delivery is at least once");
    assert_eq!(sink.effects(), 1, "the consumer applied the event once");
    db.cleanup().await;
}
