// Written by Kone Tshivhinda & Claude
//! Manual quick capture: a person describes work no connector can see.
//!
//! The caller does not build an activity event. It submits what it knows (when, what kind of work,
//! a short description) and this module derives the event's canonical fields, including the
//! idempotent id (ADR-0021). `entryId` is the caller's own idempotency key: a retry after a lost
//! response carries the same key and the same start, so it lands on the same row.
//!
//! Only what a person typed is stored, and only as subject metadata: a quick capture is never
//! content capture, so `privacy.contentCaptured` is false.

use serde::Deserialize;
use time::OffsetDateTime;
use timelex_identity::{TenantId, UserId};
use timelex_types::{
    ActivityEvent, ActivityKind, Actor as EventActor, Privacy, Source, SourceSystem, Subject,
    ACTIVITY_EVENT_SCHEMA_VERSION,
};
use uuid::Uuid;

use crate::error::StoreError;

/// Largest description accepted, in characters. It becomes `subject.title`, whose contract limit
/// is 500.
pub const MAX_DESCRIPTION_CHARS: usize = 500;

/// The kinds of work a person captures by hand. Everything else has an automated source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualKind {
    Call,
    Meeting,
    ManualEntry,
}

impl ManualKind {
    fn activity_kind(self) -> ActivityKind {
        match self {
            Self::Call => ActivityKind::Call,
            Self::Meeting => ActivityKind::Meeting,
            Self::ManualEntry => ActivityKind::ManualEntry,
        }
    }
}

/// One submitted entry. Unknown fields are refused so a client typo cannot be silently ignored.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    /// The caller's idempotency key. Any UUID spelling; stored in its simple form.
    pub entry_id: String,
    pub kind: ManualKind,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    #[serde(default)]
    pub description: Option<String>,
}

/// Turn one submitted entry into the activity event it describes.
pub fn build_event(
    tenant: TenantId,
    user: UserId,
    entry: Entry,
    captured_at: OffsetDateTime,
) -> Result<ActivityEvent, StoreError> {
    if entry.ended_at < entry.started_at {
        return Err(StoreError::Invalid("endedAt is before startedAt"));
    }
    let entry_id =
        Uuid::parse_str(&entry.entry_id).map_err(|_| StoreError::Invalid("entryId must be a UUID"))?;
    let title = entry
        .description
        .as_deref()
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "));
    if let Some(title) = &title {
        if title.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(StoreError::Invalid("description must be at most 500 characters"));
        }
    }

    let mut event = ActivityEvent {
        schema_version: ACTIVITY_EVENT_SCHEMA_VERSION.to_owned(),
        id: String::new(),
        tenant_id: format!("tn_{}", tenant.to_hex32()),
        source: Source {
            system: SourceSystem::Manual,
            source_id: entry_id.simple().to_string(),
        },
        kind: entry.kind.activity_kind(),
        actor: EventActor {
            user_id: format!("us_{}", user.as_uuid().simple()),
        },
        started_at: entry.started_at,
        ended_at: entry.ended_at,
        captured_at,
        subject: title.filter(|t| !t.is_empty()).map(|title| Subject {
            title: Some(title),
            ..Subject::default()
        }),
        content_ref: None,
        privacy: Privacy {
            content_captured: false,
            redaction_applied: false,
        },
    };
    event.id = event.expected_id();
    Ok(event)
}
