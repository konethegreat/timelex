// Written by Kone Tshivhinda & Claude
//! P2-08: a manual entry becomes one activity event. These tests pin the normalisation the HTTP
//! layer relies on; the database round trip is in `http.rs`.

use time::macros::datetime;
use timelex_capture_service::manual::{build_event, Entry, ManualKind, MAX_DESCRIPTION_CHARS};
use timelex_capture_service::StoreError;
use timelex_identity::{TenantId, UserId};
use timelex_types::{ActivityKind, SourceSystem};
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(0x0191a2b3c4d57e6f8a9b0c1d2e3f4a5b);
const USER: Uuid = Uuid::from_u128(0x0191a2b3c4d57e6f8a9b0c1d2e3f4a5c);

fn tenant() -> TenantId {
    TenantId::from_uuid(TENANT)
}

fn user() -> UserId {
    UserId::from_uuid(USER)
}

fn entry(kind: ManualKind, description: Option<&str>) -> Entry {
    Entry {
        entry_id: Uuid::from_u128(7).to_string(),
        kind,
        started_at: datetime!(2026-11-02 09:00:00 UTC),
        ended_at: datetime!(2026-11-02 09:20:00 UTC),
        description: description.map(str::to_owned),
    }
}

fn build(entry: Entry) -> timelex_types::ActivityEvent {
    build_event(tenant(), user(), entry, datetime!(2026-11-02 09:21:00 UTC)).unwrap()
}

#[test]
fn a_manual_entry_becomes_an_activity_event() {
    let event = build(entry(ManualKind::Call, Some("Call with the Deeds Office")));

    assert_eq!(event.schema_version, timelex_types::ACTIVITY_EVENT_SCHEMA_VERSION);
    assert_eq!(event.tenant_id, format!("tn_{}", tenant().to_hex32()));
    assert_eq!(event.actor.user_id, format!("us_{}", user().as_uuid().simple()));
    assert_eq!(event.source.system, SourceSystem::Manual);
    assert_eq!(event.source.source_id, Uuid::from_u128(7).simple().to_string());
    assert_eq!(event.kind, ActivityKind::Call);
    assert_eq!(event.started_at, datetime!(2026-11-02 09:00:00 UTC));
    assert_eq!(event.ended_at, datetime!(2026-11-02 09:20:00 UTC));
    assert_eq!(event.captured_at, datetime!(2026-11-02 09:21:00 UTC));
    assert_eq!(
        event.subject.as_ref().and_then(|s| s.title.as_deref()),
        Some("Call with the Deeds Office")
    );
    assert_eq!(event.content_ref, None);
    assert!(!event.privacy.content_captured);
    assert!(!event.privacy.redaction_applied);
    assert_eq!(event.id, event.expected_id());
}

#[test]
fn a_retry_of_the_same_entry_is_the_same_signal() {
    let first = build(entry(ManualKind::Meeting, Some("Consultation")));
    let second = build(entry(ManualKind::Meeting, Some("Consultation")));

    assert_eq!(first.id, second.id);
}

#[test]
fn the_three_manual_kinds_keep_their_meaning() {
    for (kind, expected) in [
        (ManualKind::Call, ActivityKind::Call),
        (ManualKind::Meeting, ActivityKind::Meeting),
        (ManualKind::ManualEntry, ActivityKind::ManualEntry),
    ] {
        assert_eq!(build(entry(kind, None)).kind, expected);
    }
}

#[test]
fn the_description_is_trimmed_to_one_line() {
    let event = build(entry(
        ManualKind::ManualEntry,
        Some("  Reviewed\t the   pleadings\nand the notice of motion  "),
    ));
    assert_eq!(
        event.subject.as_ref().and_then(|s| s.title.as_deref()),
        Some("Reviewed the pleadings and the notice of motion")
    );
}

#[test]
fn a_blank_description_is_no_title_at_all() {
    for blank in ["", "   ", "\n\t"] {
        assert_eq!(build(entry(ManualKind::ManualEntry, Some(blank))).subject, None);
    }
}

#[test]
fn an_end_before_the_start_is_refused() {
    let mut bad = entry(ManualKind::Call, None);
    bad.ended_at = datetime!(2026-11-02 08:59:59 UTC);
    let error = build_event(tenant(), user(), bad, datetime!(2026-11-02 09:00:00 UTC)).unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[test]
fn an_entry_id_that_is_not_a_uuid_is_refused() {
    let mut bad = entry(ManualKind::Call, None);
    bad.entry_id = "not-a-uuid".into();
    let error = build_event(tenant(), user(), bad, datetime!(2026-11-02 09:00:00 UTC)).unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[test]
fn a_description_over_the_limit_is_refused() {
    let long = "a".repeat(MAX_DESCRIPTION_CHARS + 1);
    let error = build_event(
        tenant(),
        user(),
        entry(ManualKind::ManualEntry, Some(&long)),
        datetime!(2026-11-02 09:00:00 UTC),
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[test]
fn a_description_at_the_limit_is_kept() {
    let exact = "a".repeat(MAX_DESCRIPTION_CHARS);
    let event = build(entry(ManualKind::ManualEntry, Some(&exact)));
    assert_eq!(
        event
            .subject
            .as_ref()
            .and_then(|s| s.title.as_deref())
            .map(str::len),
        Some(MAX_DESCRIPTION_CHARS)
    );
}
