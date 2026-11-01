// Written by Kone Tshivhinda & Claude
//! The capture store: the only way in or out of `capture.activity_events` and the connector
//! cursors.
//!
//! Every query runs inside a [`TenantTx`](timelex_db::TenantTx), so Postgres row-level security
//! confines it to one firm. A write and its `activity.recorded` outbox event share one transaction,
//! so the stream cannot describe an event that was not stored (ADR-0014).
//!
//! [`CaptureStore::ingest`] is the public path and checks `capture:write`. [`CaptureStore::record`]
//! and [`CaptureStore::record_page`] are for connectors, which run inside the service and have no
//! user token. A page's events and the cursor that follows them are written together: either every
//! event is stored and the cursor moves, or nothing changes.

use std::sync::Arc;

use time::OffsetDateTime;
use timelex_auth::Principal;
use timelex_db::{TenantPool, TenantTx};
use timelex_identity::{require_permission, Permission, TenantId, UserId};
use timelex_outbox as outbox;
use timelex_outbox::EventId;
use timelex_types::{supports_activity_schema_version, ActivityEvent};

use crate::error::StoreError;

/// A source of "now". Injected so tests can move time.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// The outbox event type of a newly recorded activity event.
pub const ACTIVITY_RECORDED: &str = "activity.recorded";

/// Who is acting, for operations that need a caller. Built from a verified internal `principal`
/// token; the store trusts nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// The firm.
    pub tenant: TenantId,
    /// The user acting.
    pub user: UserId,
    /// What they may do.
    pub permissions: std::collections::BTreeSet<Permission>,
}

impl From<&Principal> for Actor {
    fn from(principal: &Principal) -> Self {
        Self {
            tenant: principal.tenant,
            user: principal.user,
            permissions: principal.permissions.clone(),
        }
    }
}

/// Whether an ingest stored a new event or found the one already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    Created,
    Existing,
}

/// The result of one ingest. `id` is the event's canonical id, which a duplicate delivery also gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingested {
    pub id: String,
    pub outcome: IngestOutcome,
}

/// What one page of connector events did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageOutcome {
    pub created: usize,
    pub existing: usize,
}

/// The capture store.
#[derive(Clone)]
pub struct CaptureStore {
    pool: TenantPool,
    clock: Clock,
}

impl CaptureStore {
    /// A store over the service's connection pool, using the system clock.
    pub fn new(pool: TenantPool) -> Self {
        Self::with_clock(pool, Arc::new(OffsetDateTime::now_utc))
    }

    /// A store with an injected clock. Tests use this to make `recorded_at` deterministic.
    pub fn with_clock(pool: TenantPool, clock: Clock) -> Self {
        Self { pool, clock }
    }

    /// Check the database answers. For `/health`; reads no tenant data.
    pub async fn ping(&self) -> Result<(), StoreError> {
        Ok(self.pool.ping().await?)
    }

    /// `POST /v1/events`: store one event submitted by a principal with `capture:write`.
    pub async fn ingest(&self, actor: Actor, event: ActivityEvent) -> Result<Ingested, StoreError> {
        require_permission(&actor.permissions, Permission::CaptureWrite)
            .map_err(|_| StoreError::Forbidden)?;
        self.record(actor.tenant, event).await
    }

    /// Store one event on behalf of a connector. No permission check: this is the service's own work,
    /// not a request from a user.
    pub async fn record(&self, tenant: TenantId, event: ActivityEvent) -> Result<Ingested, StoreError> {
        validate(tenant, &event)?;
        let mut tx = self.pool.begin(tenant).await?;
        let created = insert_event(&mut tx, &event, (self.clock)()).await?;
        tx.commit().await?;
        Ok(Ingested {
            id: event.id,
            outcome: if created {
                IngestOutcome::Created
            } else {
                IngestOutcome::Existing
            },
        })
    }

    /// Store one page of connector events and, if it has a next cursor, move the cursor — all in one
    /// transaction. An invalid event fails the page before anything is written, so the connector
    /// re-reads it on the next run instead of skipping it.
    pub async fn record_page(
        &self,
        tenant: TenantId,
        connector: &str,
        events: &[ActivityEvent],
        next: Option<&str>,
    ) -> Result<PageOutcome, StoreError> {
        validate_connector(connector)?;
        if let Some(cursor) = next {
            validate_cursor(cursor)?;
        }
        for event in events {
            validate(tenant, event)?;
        }
        let at = (self.clock)();
        let mut tx = self.pool.begin(tenant).await?;
        let mut created = 0;
        for event in events {
            if insert_event(&mut tx, event, at).await? {
                created += 1;
            }
        }
        if let Some(cursor) = next {
            save_cursor(&mut tx, tenant, connector, cursor, at).await?;
        }
        tx.commit().await?;
        Ok(PageOutcome {
            created,
            existing: events.len() - created,
        })
    }

    /// The stored cursor for a connector, or `None` if it has never completed a page.
    pub async fn cursor(&self, tenant: TenantId, connector: &str) -> Result<Option<String>, StoreError> {
        validate_connector(connector)?;
        let mut tx = self.pool.begin(tenant).await?;
        let cursor: Option<String> =
            sqlx::query_scalar("SELECT cursor FROM capture.connector_cursors WHERE connector = $1")
                .bind(connector)
                .fetch_optional(tx.conn())
                .await?;
        tx.commit().await?;
        Ok(cursor)
    }
}

/// The rules an event must pass before it is stored. The id is checked against its content, so a
/// client cannot choose an id and a producer cannot drift from the derivation.
fn validate(tenant: TenantId, event: &ActivityEvent) -> Result<(), StoreError> {
    if event.tenant_id != format!("tn_{}", tenant.to_hex32()) {
        return Err(StoreError::Invalid("the event belongs to a different tenant"));
    }
    if !supports_activity_schema_version(&event.schema_version) {
        return Err(StoreError::Invalid(
            "schemaVersion is not a supported 1.x activity-event version",
        ));
    }
    if event.id != event.expected_id() {
        return Err(StoreError::Invalid("the event id does not match its content"));
    }
    if event.ended_at < event.started_at {
        return Err(StoreError::Invalid("endedAt is before startedAt"));
    }
    if event.source.source_id.is_empty() || event.source.source_id.chars().count() > 512 {
        return Err(StoreError::Invalid("source.sourceId must be 1 to 512 characters"));
    }
    let actor = event.actor.user_id.strip_prefix("us_").unwrap_or("");
    if !(8..=64).contains(&actor.chars().count()) || !actor.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(StoreError::Invalid(
            "actor.userId must be us_ followed by 8 to 64 letters or digits",
        ));
    }
    Ok(())
}

fn validate_connector(connector: &str) -> Result<(), StoreError> {
    let mut bytes = connector.bytes();
    let ok = bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && connector.len() <= 64
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !ok {
        return Err(StoreError::Invalid(
            "connector must be a lowercase name of at most 64 characters",
        ));
    }
    Ok(())
}

fn validate_cursor(cursor: &str) -> Result<(), StoreError> {
    if cursor.is_empty() || cursor.chars().count() > 4096 {
        return Err(StoreError::Invalid("cursor must be 1 to 4096 characters"));
    }
    Ok(())
}

/// Insert one event. `false` means the id was already stored: the signal was delivered before.
async fn insert_event(
    tx: &mut TenantTx,
    event: &ActivityEvent,
    recorded_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let payload = serde_json::to_value(event)
        .map_err(|e| StoreError::Internal(format!("the event would not serialise: {e}")))?;
    let inserted = sqlx::query(
        "INSERT INTO capture.activity_events \
         (tenant_id, id, source_system, source_id, actor_id, kind, started_at, ended_at, \
          captured_at, recorded_at, event) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         ON CONFLICT (tenant_id, id) DO NOTHING",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&event.id)
    .bind(event.source.system.as_str())
    .bind(&event.source.source_id)
    .bind(&event.actor.user_id)
    .bind(event.kind.as_str())
    .bind(event.started_at)
    .bind(event.ended_at)
    .bind(event.captured_at)
    .bind(recorded_at)
    .bind(&payload)
    .execute(tx.conn())
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(false);
    }
    let tenant = tx.tenant();
    outbox::append(
        tx.conn(),
        &outbox::Event {
            id: EventId::new(),
            tenant,
            event_type: ACTIVITY_RECORDED.to_owned(),
            occurred_at: recorded_at,
            payload,
        },
    )
    .await?;
    Ok(true)
}

async fn save_cursor(
    tx: &mut TenantTx,
    tenant: TenantId,
    connector: &str,
    cursor: &str,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO capture.connector_cursors (tenant_id, connector, cursor, updated_at) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (tenant_id, connector) \
         DO UPDATE SET cursor = EXCLUDED.cursor, updated_at = EXCLUDED.updated_at",
    )
    .bind(tenant.as_uuid())
    .bind(connector)
    .bind(cursor)
    .bind(at)
    .execute(tx.conn())
    .await?;
    Ok(())
}
