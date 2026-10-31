// Written by Kone Tshivhinda & Claude
//! The transactional outbox (P1-06, ADR-0010, ADR-0014).
//!
//! A service writes an event in the **same transaction** as the state change it describes, so the
//! two cannot disagree. A [`Relay`] claims unpublished rows with `FOR UPDATE SKIP LOCKED`, hands each
//! to a [`Publisher`], and records the result. Delivery is at least once: a crash between publishing
//! and recording republishes, and consumers dedupe on the event id (the test in `tests/relay.rs`
//! shows one effect for a redelivered event).
//!
//! The queue is `platform.outbox`, one table for every service. Why it is not in a service schema,
//! and why it has no row-level security, is recorded in ADR-0014. This crate never puts a secret in
//! an event; that is the contract's job (`contracts/events/secret-event.schema.json`).

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::{FromRow, PgConnection, PgPool};
use time::OffsetDateTime;
use timelex_identity::TenantId;
use uuid::Uuid;

/// An event's identity. Delivery is at least once, so consumers record the ids they have applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(Uuid);

impl EventId {
    /// A fresh random id.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// The underlying UUID (the queue's `id` column).
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    /// `ev_` followed by 32 lowercase hex characters, as the contract requires.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ev_{}", self.0.simple())
    }
}

/// One event to append. `payload` is the complete contract envelope, serialised.
#[derive(Debug, Clone)]
pub struct Event {
    pub id: EventId,
    pub tenant: TenantId,
    /// For operators and indexes; the payload is the source of truth.
    pub event_type: String,
    /// The instant the event happened, also used for the queue's `created_at`.
    pub occurred_at: OffsetDateTime,
    pub payload: Value,
}

/// Append an event inside the caller's transaction.
///
/// Take the same transaction as the state change. If that transaction rolls back, the event does
/// too; there is no second write that could fail on its own.
pub async fn append(conn: &mut PgConnection, event: &Event) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO platform.outbox (id, tenant_id, event_type, payload, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event.id.as_uuid())
    .bind(event.tenant.as_uuid())
    .bind(&event.event_type)
    .bind(&event.payload)
    .bind(event.occurred_at)
    .execute(conn)
    .await
    .map(|_| ())
}

/// An event claimed by a relay. The publisher sees the envelope and its metadata.
#[derive(Debug, Clone)]
pub struct QueuedEvent {
    pub id: Uuid,
    pub tenant: TenantId,
    pub event_type: String,
    pub payload: Value,
    /// How many times delivery has been attempted, including this one.
    pub attempts: i32,
}

/// Where a relay sends events. Phase 1 has a JSON-line publisher; a broker publisher slots in here.
#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish(&self, event: &QueuedEvent) -> Result<(), String>;
}

/// Writes each event envelope as one JSON line. The interim sink until a broker exists, and the
/// same shape the gateway uses for metering.
pub struct JsonLinePublisher {
    sink: Box<dyn Fn(&str) + Send + Sync>,
}

impl JsonLinePublisher {
    /// The production publisher: one JSON line per event on stdout.
    pub fn stdout() -> Self {
        Self::with_sink(|line| println!("{line}"))
    }

    /// A publisher that hands each line to `sink`. Used by tests to keep stdout quiet.
    pub fn with_sink(sink: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self { sink: Box::new(sink) }
    }
}

#[async_trait]
impl Publisher for JsonLinePublisher {
    async fn publish(&self, event: &QueuedEvent) -> Result<(), String> {
        let line = serde_json::to_string(&event.payload).map_err(|e| e.to_string())?;
        (self.sink)(&line);
        Ok(())
    }
}

/// The row a claim query returns.
#[derive(FromRow)]
struct Claimed {
    id: Uuid,
    tenant_id: Uuid,
    event_type: String,
    payload: Value,
    attempts: i32,
}

/// Claims due events and delivers them. Safe to run in more than one process: a claim takes a row
/// lock with `SKIP LOCKED`, and the claimed row is not due again until its lease expires.
pub struct Relay<P> {
    pool: PgPool,
    publisher: P,
}

/// How long a claim is reserved before a crashed relay's row is retried.
const CLAIM_LEASE_SECONDS: f64 = 60.0;

/// The cap on the exponential retry delay.
const MAX_BACKOFF_SECONDS: f64 = 300.0;

impl<P: Publisher> Relay<P> {
    pub fn new(pool: PgPool, publisher: P) -> Self {
        Self { pool, publisher }
    }

    /// Claim up to `limit` due events, publish each, and record the result. Returns how many rows
    /// were claimed. A publisher failure is recorded on the row and does not stop the others; only a
    /// database failure is returned.
    pub async fn poll_once(&self, limit: i64) -> Result<usize, sqlx::Error> {
        let claimed: Vec<Claimed> = sqlx::query_as(
            "UPDATE platform.outbox \
                SET attempts = attempts + 1, \
                    next_attempt_at = now() + make_interval(secs => $2) \
              WHERE id IN ( \
                    SELECT id FROM platform.outbox \
                     WHERE published_at IS NULL AND next_attempt_at <= now() \
                     ORDER BY seq \
                     LIMIT $1 \
                     FOR UPDATE SKIP LOCKED) \
              RETURNING id, tenant_id, event_type, payload, attempts",
        )
        .bind(limit)
        .bind(CLAIM_LEASE_SECONDS)
        .fetch_all(&self.pool)
        .await?;

        let count = claimed.len();
        for row in claimed {
            let event = QueuedEvent {
                id: row.id,
                tenant: TenantId::from_uuid(row.tenant_id),
                event_type: row.event_type,
                payload: row.payload,
                attempts: row.attempts,
            };
            match self.publisher.publish(&event).await {
                Ok(()) => {
                    sqlx::query(
                        "UPDATE platform.outbox SET published_at = now(), last_error = NULL \
                          WHERE id = $1",
                    )
                    .bind(event.id)
                    .execute(&self.pool)
                    .await?;
                }
                Err(message) => {
                    let seconds = backoff_seconds(event.attempts);
                    eprintln!(
                        "outbox relay: event {} ({}) failed (attempt {}): {message}",
                        event.id, event.event_type, event.attempts
                    );
                    sqlx::query(
                        "UPDATE platform.outbox \
                            SET last_error = $2, next_attempt_at = now() + make_interval(secs => $3) \
                          WHERE id = $1",
                    )
                    .bind(event.id)
                    .bind(message)
                    .bind(seconds)
                    .execute(&self.pool)
                    .await?;
                }
            }
        }
        Ok(count)
    }

    /// Poll forever, waiting `interval` between empty polls. A poll error is logged and retried; it
    /// never stops the relay.
    pub async fn run(&self, interval: Duration) {
        loop {
            if let Err(e) = self.poll_once(20).await {
                eprintln!("outbox relay: poll failed: {e}");
            }
            tokio::time::sleep(interval).await;
        }
    }
}

/// 2^attempts seconds, capped, so a sick subscriber is retried without hammering it.
fn backoff_seconds(attempts: i32) -> f64 {
    2.0_f64.powi(attempts.clamp(1, 20)).min(MAX_BACKOFF_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_event_id_is_ev_and_32_lowercase_hex() {
        let id = EventId::new().to_string();
        assert!(id.starts_with("ev_"), "{id}");
        assert_eq!(id.len(), 35, "{id}");
        assert!(
            id[3..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{id}"
        );
        assert_ne!(EventId::new().to_string(), id, "ids are random");
    }

    #[test]
    fn retry_delay_grows_and_is_capped() {
        assert_eq!(backoff_seconds(1), 2.0);
        assert_eq!(backoff_seconds(2), 4.0);
        assert_eq!(backoff_seconds(8), 256.0);
        assert_eq!(backoff_seconds(20), MAX_BACKOFF_SECONDS);
    }
}
