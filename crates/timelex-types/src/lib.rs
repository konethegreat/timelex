// Written by Kone Tshivhinda & Claude
//! Rust mirror of the contracts in `contracts/`.
//!
//! The JSON Schema is the source of truth. The tests in this crate validate the golden example
//! against the schema, deserialise it into these types, serialise it back and validate again, so
//! a change to either side that breaks the other fails CI (ADR-0002 rule 3).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// Contract version implemented by this crate.
pub const ACTIVITY_EVENT_SCHEMA_VERSION: &str = "1.0";

/// Domain separation for the v1 activity-event id. The trailing NUL cannot appear in any input.
const ACTIVITY_EVENT_ID_DOMAIN: &[u8] = b"timelex/activity-event/id/v1\0";

/// Whether a `schemaVersion` is a 1.x activity-event version this build can read. A newer minor is
/// accepted: readers ignore fields they do not know (see `contracts/events/README.md`).
pub fn supports_activity_schema_version(version: &str) -> bool {
    version
        .strip_prefix("1.")
        .is_some_and(|minor| !minor.is_empty() && minor.bytes().all(|b| b.is_ascii_digit()))
}

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

impl SourceSystem {
    /// The contract's spelling, also the input to the id rule.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M365Mail => "m365_mail",
            Self::M365Calendar => "m365_calendar",
            Self::M365Teams => "m365_teams",
            Self::GoogleMail => "google_mail",
            Self::GoogleCalendar => "google_calendar",
            Self::DesktopAgent => "desktop_agent",
            Self::Manual => "manual",
            Self::Import => "import",
        }
    }
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

impl ActivityKind {
    /// The contract's spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmailSent => "email_sent",
            Self::EmailReceived => "email_received",
            Self::Meeting => "meeting",
            Self::Call => "call",
            Self::DocumentEdit => "document_edit",
            Self::DocumentView => "document_view",
            Self::WebResearch => "web_research",
            Self::DesktopFocus => "desktop_focus",
            Self::ManualEntry => "manual_entry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub system: SourceSystem,
    pub source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub role: ParticipantRole,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct Privacy {
    pub content_captured: bool,
    pub redaction_applied: bool,
}

/// Immutable fact about one observed work signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    /// The id this event's content requires. A producer and capture-hub both compute it, so the same
    /// signal delivered twice lands on the same row.
    pub fn expected_id(&self) -> String {
        activity_event_id(&self.tenant_id, &self.source, self.started_at)
    }
}

/// The canonical, idempotent id of one activity event.
///
/// SHA-256 over a domain separator and the four facts that identify a signal, each written as a
/// 4-byte big-endian length followed by its bytes, then `startedAt` as an 8-byte big-endian Unix
/// timestamp in whole seconds (sub-second precision is truncated). The first 16 bytes, lowercase hex,
/// carry the `ae_` prefix.
///
/// Length prefixes mean no boundary can shift: tenant `ab` + source `c` is not tenant `a` + source
/// `bc`. Kind, actor and capture time are observations about the signal, not its identity, so they do
/// not take part. The exact rule is stated in `contracts/events/README.md`; changing it would change
/// every id, so it is versioned with the contract.
pub fn activity_event_id(tenant_id: &str, source: &Source, started_at: OffsetDateTime) -> String {
    let mut h = Sha256::new();
    h.update(ACTIVITY_EVENT_ID_DOMAIN);
    for part in [tenant_id, source.system.as_str(), source.source_id.as_str()] {
        let bytes = part.as_bytes();
        h.update(u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes());
        h.update(bytes);
    }
    h.update(started_at.unix_timestamp().to_be_bytes());
    let digest = h.finalize();
    format!("ae_{}", hex(&digest[..16]))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
mod id_tests {
    use super::*;
    use serde_json::Value;
    use time::macros::datetime;

    const EXAMPLE: &str = include_str!("../../../contracts/examples/activity-event.json");

    fn example() -> ActivityEvent {
        serde_json::from_str(EXAMPLE).expect("example deserialises")
    }

    #[test]
    fn the_same_signal_always_has_the_same_id() {
        let e = example();
        assert_eq!(e.expected_id(), e.expected_id());
        assert_eq!(
            activity_event_id(&e.tenant_id, &e.source, e.started_at),
            e.expected_id()
        );
    }

    #[test]
    fn the_id_is_the_documented_shape() {
        let id = example().expected_id();
        assert!(id.starts_with("ae_"), "{id}");
        let hex = id.strip_prefix("ae_").unwrap();
        assert_eq!(hex.len(), 32, "16 bytes of SHA-256, lowercase hex");
        assert!(hex
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }

    #[test]
    fn the_golden_id_is_stable() {
        // Pinned so an accidental change to the encoding (field order, separator, time unit)
        // breaks this test. The value was computed independently from the documented layout.
        assert_eq!(example().expected_id(), "ae_819918eaf3953eb60573f889187a7b92");
    }

    #[test]
    fn changing_any_of_the_four_facts_changes_the_id() {
        let base = example();
        let base_id = base.expected_id();

        let mut other_tenant = base.clone();
        other_tenant.tenant_id = "tn_other0002".into();
        assert_ne!(other_tenant.expected_id(), base_id, "tenant");

        let mut other_system = base.clone();
        other_system.source.system = SourceSystem::GoogleMail;
        assert_ne!(other_system.expected_id(), base_id, "source system");

        let mut other_source = base.clone();
        other_source.source.source_id = "AAMkAGI2another-message-id".into();
        assert_ne!(other_source.expected_id(), base_id, "source id");

        let mut other_start = base.clone();
        other_start.started_at += time::Duration::seconds(1);
        assert_ne!(other_start.expected_id(), base_id, "started at");
    }

    #[test]
    fn facts_outside_the_rule_do_not_change_the_id() {
        let base = example();
        let base_id = base.expected_id();

        let mut other_kind = base.clone();
        other_kind.kind = ActivityKind::Meeting;
        assert_eq!(other_kind.expected_id(), base_id, "kind is not part of the rule");

        let mut other_capture = base.clone();
        other_capture.captured_at += time::Duration::seconds(90);
        assert_eq!(other_capture.expected_id(), base_id, "capture time is not either");

        let mut other_id = base.clone();
        other_id.id = "ae_00000000000000000000000000000000".into();
        assert_eq!(
            other_id.expected_id(),
            base_id,
            "the stored id is replaced, not read"
        );
    }

    #[test]
    fn sub_second_precision_is_truncated_to_whole_seconds() {
        let base = example();
        let mut rounded = base.clone();
        rounded.started_at = rounded
            .started_at
            .replace_nanosecond(999_999_000)
            .expect("valid nanosecond");
        assert_eq!(rounded.expected_id(), base.expected_id());
    }

    #[test]
    fn fields_are_length_prefixed_so_boundaries_cannot_shift() {
        // "ab" + "c" and "a" + "bc" must not produce the same digest.
        let started = datetime!(2026-10-20 08:12:00 UTC);
        let a = activity_event_id(
            "tn_ab",
            &Source {
                system: SourceSystem::Manual,
                source_id: "c".into(),
            },
            started,
        );
        let b = activity_event_id(
            "tn_a",
            &Source {
                system: SourceSystem::Manual,
                source_id: "bc".into(),
            },
            started,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn the_generated_id_satisfies_the_contract_pattern() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/events/activity-event.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut value = serde_json::to_value(example()).unwrap();
        value["id"] = Value::String(example().expected_id());
        assert!(validator.is_valid(&value), "{value}");
    }

    #[test]
    fn every_kind_has_the_contract_spelling() {
        use ActivityKind::*;
        for kind in [
            EmailSent,
            EmailReceived,
            Meeting,
            Call,
            DocumentEdit,
            DocumentView,
            WebResearch,
            DesktopFocus,
            ManualEntry,
        ] {
            let json = serde_json::to_value(kind).unwrap();
            assert_eq!(json, Value::String(kind.as_str().into()), "{kind:?}");
        }
    }

    #[test]
    fn only_a_1_x_version_is_supported() {
        for good in ["1.0", "1.1", "1.42"] {
            assert!(supports_activity_schema_version(good), "{good}");
        }
        for bad in ["", "1", "1.", "1.x", "2.0", "0.9", " 1.0", "1.0.1", "v1.0"] {
            assert!(!supports_activity_schema_version(bad), "{bad:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SCHEMA: &str = include_str!("../../../contracts/events/activity-event.schema.json");
    const EXAMPLE: &str = include_str!("../../../contracts/examples/activity-event.json");
    const RELEASED_V1_0: &str = include_str!("../../../contracts/examples/activity-event.v1.0.json");

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
    fn schema_rejects_a_bad_enum_value() {
        let mut bad_kind: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_kind["kind"] = Value::String("teleport".into());
        assert!(!validator().is_valid(&bad_kind));
    }

    #[test]
    fn the_released_v1_0_event_stays_valid_and_readable() {
        let instance: Value = serde_json::from_str(RELEASED_V1_0).unwrap();
        assert!(
            validator().is_valid(&instance),
            "a released 1.0 event must stay valid under the current schema"
        );
        let event: ActivityEvent =
            serde_json::from_str(RELEASED_V1_0).expect("a released 1.0 event must stay readable");
        assert_eq!(event.schema_version, "1.0");
        assert_eq!(event.kind, ActivityKind::EmailSent);
        assert_eq!(event.duration_seconds(), 8 * 60);
    }

    #[test]
    fn a_newer_minor_with_unknown_fields_is_readable() {
        let mut newer: Value = serde_json::from_str(RELEASED_V1_0).unwrap();
        newer["schemaVersion"] = Value::String("1.1".into());
        newer["capturedBy"] = Value::String("capture-hub".into());
        newer["source"]["mailboxRef"] = Value::String("mb_synthetic".into());
        newer["subject"]["classification"] = Value::String("client".into());
        newer["subject"]["participants"][0]["displayName"] = Value::String("Fee Earner".into());
        newer["privacy"]["tier"] = Value::String("metadata".into());
        assert!(
            validator().is_valid(&newer),
            "a newer 1.x event with unknown fields must validate: {newer}"
        );

        // An older reader keeps the fields it knows and drops the rest without guessing.
        let event: ActivityEvent = serde_json::from_value(newer).expect("a newer 1.x event must deserialise");
        assert_eq!(event.schema_version, "1.1");
        assert_eq!(event.kind, ActivityKind::EmailSent);
        assert_eq!(event.source.system, SourceSystem::M365Mail);
        assert_eq!(event.subject.as_ref().unwrap().participants.len(), 2);

        let back = serde_json::to_value(&event).unwrap();
        assert!(validator().is_valid(&back));
        assert!(back.get("capturedBy").is_none(), "unknown fields are dropped");
        assert!(back["source"].get("mailboxRef").is_none());
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
    fn rust_types_reject_a_bad_enum_value_like_the_schema_does() {
        let mut bad_kind: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_kind["kind"] = Value::String("teleport".into());
        assert!(serde_json::from_value::<ActivityEvent>(bad_kind).is_err());
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
    const RELEASED_V1_0: &str = include_str!("../../../contracts/examples/secret-event.v1.0.json");

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
    fn schema_rejects_a_missing_required_field_and_a_bad_kind() {
        let mut missing: Value = serde_json::from_str(EXAMPLE).unwrap();
        missing.as_object_mut().unwrap().remove("tenantId");
        assert!(!validator().is_valid(&missing));

        let mut bad_kind: Value = serde_json::from_str(EXAMPLE).unwrap();
        bad_kind["data"]["kind"] = Value::String("teleport".into());
        assert!(!validator().is_valid(&bad_kind));
    }

    #[test]
    fn the_released_v1_0_event_stays_valid_and_readable() {
        let instance: Value = serde_json::from_str(RELEASED_V1_0).unwrap();
        assert!(
            validator().is_valid(&instance),
            "a released 1.0 event must stay valid under the current schema"
        );
        let event: SecretEvent =
            serde_json::from_str(RELEASED_V1_0).expect("a released 1.0 event must stay readable");
        assert_eq!(event.schema_version, "1.0");
        assert_eq!(event.event_type, SecretEventType::Tested);
    }

    #[test]
    fn a_newer_minor_with_unknown_fields_is_readable() {
        let mut newer: Value = serde_json::from_str(RELEASED_V1_0).unwrap();
        newer["schemaVersion"] = Value::String("1.1".into());
        newer["traceId"] = Value::String("synthetic-trace".into());
        newer["actor"]["service"] = Value::String("vault".into());
        newer["data"]["rotatedFrom"] = Value::String("synthetic".into());
        assert!(
            validator().is_valid(&newer),
            "a newer 1.x event with unknown fields must validate: {newer}"
        );

        let event: SecretEvent = serde_json::from_value(newer).expect("a newer 1.x event must deserialise");
        assert_eq!(event.schema_version, "1.1");
        assert_eq!(event.event_type, SecretEventType::Tested);
        assert_eq!(event.data.result, Some(SecretTestResult::Passed));

        let back = serde_json::to_value(&event).unwrap();
        assert!(validator().is_valid(&back));
        assert!(back.get("traceId").is_none(), "unknown fields are dropped");
        assert!(back["data"].get("rotatedFrom").is_none());
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
