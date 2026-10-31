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

/// Contract version of the secret events in `contracts/events/secret-event.schema.json`.
pub const SECRET_EVENT_SCHEMA_VERSION: &str = "1.0";

/// What happened to a provider secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretEventType {
    #[serde(rename = "secret.created")]
    Created,
    #[serde(rename = "secret.updated")]
    Updated,
    #[serde(rename = "secret.tested")]
    Tested,
    #[serde(rename = "secret.rotated")]
    Rotated,
    #[serde(rename = "secret.revoked")]
    Revoked,
}

impl SecretEventType {
    /// The stored and displayed spelling, also used as the outbox `event_type`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "secret.created",
            Self::Updated => "secret.updated",
            Self::Tested => "secret.tested",
            Self::Rotated => "secret.rotated",
            Self::Revoked => "secret.revoked",
        }
    }
}

/// How the provider check ended. Only `secret.tested` carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretTestResult {
    Passed,
    Failed,
}

/// What the event says about the secret. Never a value: the metadata an admin already sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SecretEventData {
    pub secret_id: String,
    pub kind: String,
    pub label: String,
    pub last_four: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SecretTestResult>,
}

impl SecretEventData {
    pub fn new(
        secret_id: impl Into<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
        last_four: impl Into<String>,
        result: Option<SecretTestResult>,
    ) -> Self {
        Self {
            secret_id: secret_id.into(),
            kind: kind.into(),
            label: label.into(),
            last_four: last_four.into(),
            result,
        }
    }
}

/// Immutable fact about one change to a firm's provider secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SecretEvent {
    pub schema_version: String,
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: SecretEventType,
    pub source: String,
    pub tenant_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub time: OffsetDateTime,
    pub actor: Actor,
    pub data: SecretEventData,
}

impl SecretEvent {
    pub fn new(
        id: impl Into<String>,
        tenant_id: impl Into<String>,
        user_id: impl Into<String>,
        time: OffsetDateTime,
        event_type: SecretEventType,
        data: SecretEventData,
    ) -> Self {
        Self {
            schema_version: SECRET_EVENT_SCHEMA_VERSION.to_owned(),
            id: id.into(),
            event_type,
            source: "vault".to_owned(),
            tenant_id: tenant_id.into(),
            time,
            actor: Actor {
                user_id: user_id.into(),
            },
            data,
        }
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

#[cfg(test)]
mod secret_tests {
    use super::*;
    use serde_json::Value;
    use time::macros::datetime;

    const SCHEMA: &str = include_str!("../../../contracts/events/secret-event.schema.json");
    const EXAMPLE: &str = include_str!("../../../contracts/examples/secret-event.json");

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
        let event: SecretEvent = serde_json::from_str(EXAMPLE).expect("example deserialises");
        assert_eq!(event.schema_version, SECRET_EVENT_SCHEMA_VERSION);
        assert_eq!(event.event_type, SecretEventType::Tested);
        assert_eq!(event.source, "vault");
        assert_eq!(event.data.result, Some(SecretTestResult::Passed));
        assert_eq!(event.actor.user_id, "us_demo0001");

        let back = serde_json::to_value(&event).unwrap();
        assert!(
            validator().is_valid(&back),
            "serialised event must satisfy the schema"
        );
        let original: Value = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(back, original, "round trip must not add, drop or change fields");
    }

    #[test]
    fn a_tested_event_requires_a_result_and_an_untested_one_forbids_it() {
        let mut missing: Value = serde_json::from_str(EXAMPLE).unwrap();
        missing["data"].as_object_mut().unwrap().remove("result");
        assert!(!validator().is_valid(&missing), "tested without a result");

        let mut extra: Value = serde_json::from_str(EXAMPLE).unwrap();
        extra["type"] = Value::String("secret.created".into());
        assert!(!validator().is_valid(&extra), "created with a provider result");
    }

    #[test]
    fn schema_rejects_missing_required_field_unknown_field_and_bad_kind() {
        let mut missing: Value = serde_json::from_str(EXAMPLE).unwrap();
        missing.as_object_mut().unwrap().remove("tenantId");
        assert!(!validator().is_valid(&missing));

        let mut extra: Value = serde_json::from_str(EXAMPLE).unwrap();
        extra["surprise"] = Value::Bool(true);
        assert!(!validator().is_valid(&extra));

        let mut bad_kind: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_kind["data"]["kind"] = Value::String("teleport".into());
        assert!(!validator().is_valid(&bad_kind));
    }

    #[test]
    fn schema_rejects_bad_ids_and_a_wrong_major_version() {
        let mut bad_event_id: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_event_id["id"] = Value::String("not-an-id".into());
        assert!(!validator().is_valid(&bad_event_id));

        let mut bad_tenant: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_tenant["tenantId"] = Value::String("0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b".into());
        assert!(!validator().is_valid(&bad_tenant), "the tn_ form is required");

        let mut v2: Value = serde_json::from_str(EXAMPLE).unwrap();
        v2["schemaVersion"] = Value::String("2.0".into());
        assert!(!validator().is_valid(&v2));
    }

    #[test]
    fn rust_types_reject_unknown_fields_like_the_schema_does() {
        let mut extra: Value = serde_json::from_str(EXAMPLE).unwrap();
        extra["surprise"] = Value::Bool(true);
        assert!(serde_json::from_value::<SecretEvent>(extra).is_err());
    }

    #[test]
    fn the_constructor_produces_a_valid_event() {
        let event = SecretEvent::new(
            "ev_0123456789abcdef0123456789abcdef",
            "tn_0123456789abcdef0123456789abcdef",
            "us_fedcba9876543210fedcba9876543210",
            datetime!(2026-10-31 09:15:00 UTC),
            SecretEventType::Revoked,
            SecretEventData::new(
                "0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b",
                "anthropic_api_key",
                "Firm Claude key",
                "1234",
                None,
            ),
        );
        let value = serde_json::to_value(&event).unwrap();
        assert!(validator().is_valid(&value), "{value}");
        assert_eq!(value["source"], "vault");
        assert_eq!(value["type"], "secret.revoked");
        assert!(value["data"].get("result").is_none());
    }
}
