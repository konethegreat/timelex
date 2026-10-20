// Written by Kone Tshivhinda & Claude
//! Rust mirror of the contracts in `contracts/`.
//!
//! The JSON Schema is the source of truth. The tests in this crate validate the golden example
//! against the schema, deserialise it into these types, serialise it back and validate again, so
//! a change to either side that breaks the other fails CI (ADR-0002 rule 3).

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Contract version implemented by this crate.
pub const ACTIVITY_EVENT_SCHEMA_VERSION: &str = "1.0";

/// Which system produced the signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSystem {
    M365Mail,
    M365Calendar,
    M365Teams,
    GoogleMail,
    GoogleCalendar,
    DesktopAgent,
    Manual,
    Import,
}

/// What kind of work the signal represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    EmailSent,
    EmailReceived,
    Meeting,
    Call,
    DocumentEdit,
    DocumentView,
    WebResearch,
    DesktopFocus,
    ManualEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Source {
    pub system: SourceSystem,
    pub source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Actor {
    pub user_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    From,
    To,
    Cc,
    Attendee,
    Organizer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Participant {
    pub role: ParticipantRole,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Subject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<Participant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Privacy {
    pub content_captured: bool,
    pub redaction_applied: bool,
}

/// Immutable fact about one observed work signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ActivityEvent {
    pub schema_version: String,
    pub id: String,
    pub tenant_id: String,
    pub source: Source,
    pub kind: ActivityKind,
    pub actor: Actor,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub captured_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Subject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_ref: Option<String>,
    pub privacy: Privacy,
}

impl ActivityEvent {
    /// Whole seconds between start and end; zero if the end precedes the start.
    pub fn duration_seconds(&self) -> u64 {
        let secs = (self.ended_at - self.started_at).whole_seconds();
        u64::try_from(secs).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SCHEMA: &str = include_str!("../../../contracts/events/activity-event.schema.json");
    const EXAMPLE: &str = include_str!("../../../contracts/examples/activity-event.json");

    fn validator() -> jsonschema::Validator {
        let schema: Value = serde_json::from_str(SCHEMA).expect("schema is valid JSON");
        jsonschema::validator_for(&schema).expect("schema compiles")
    }

    #[test]
    fn golden_example_matches_schema() {
        let instance: Value = serde_json::from_str(EXAMPLE).unwrap();
        assert!(
            validator().is_valid(&instance),
            "golden example must satisfy the schema"
        );
    }

    #[test]
    fn example_round_trips_through_rust_types_and_stays_valid() {
        let event: ActivityEvent = serde_json::from_str(EXAMPLE).expect("example deserialises");
        assert_eq!(event.schema_version, ACTIVITY_EVENT_SCHEMA_VERSION);
        assert_eq!(event.kind, ActivityKind::EmailSent);
        assert_eq!(event.source.system, SourceSystem::M365Mail);
        assert_eq!(event.duration_seconds(), 8 * 60);

        let back = serde_json::to_value(&event).unwrap();
        assert!(
            validator().is_valid(&back),
            "serialised event must satisfy the schema"
        );
        let original: Value = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(back, original, "round trip must not add, drop or change fields");
    }

    #[test]
    fn schema_rejects_missing_required_field() {
        let mut instance: Value = serde_json::from_str(EXAMPLE).unwrap();
        instance.as_object_mut().unwrap().remove("tenantId");
        assert!(!validator().is_valid(&instance));
    }

    #[test]
    fn schema_rejects_unknown_field_and_bad_enum() {
        let mut extra: Value = serde_json::from_str(EXAMPLE).unwrap();
        extra["surprise"] = Value::Bool(true);
        assert!(!validator().is_valid(&extra));

        let mut bad_kind: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_kind["kind"] = Value::String("teleport".into());
        assert!(!validator().is_valid(&bad_kind));
    }

    #[test]
    fn schema_rejects_non_idempotent_looking_id_and_wrong_major_version() {
        let mut bad_id: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_id["id"] = Value::String("not-an-id".into());
        assert!(!validator().is_valid(&bad_id));

        let mut v2: Value = serde_json::from_str(EXAMPLE).unwrap();
        v2["schemaVersion"] = Value::String("2.0".into());
        assert!(!validator().is_valid(&v2));
    }

    #[test]
    fn rust_types_reject_unknown_fields_like_the_schema_does() {
        let mut extra: Value = serde_json::from_str(EXAMPLE).unwrap();
        extra["surprise"] = Value::Bool(true);
        assert!(serde_json::from_value::<ActivityEvent>(extra).is_err());
    }

    #[test]
    fn duration_is_zero_when_end_precedes_start() {
        let mut event: ActivityEvent = serde_json::from_str(EXAMPLE).unwrap();
        std::mem::swap(&mut event.started_at, &mut event.ended_at);
        assert_eq!(event.duration_seconds(), 0);
    }
}
