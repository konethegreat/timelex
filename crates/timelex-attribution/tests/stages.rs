// Written by Kone Tshivhinda & Claude
//! Stage 0 (hard keys) and stage 1 (history) attribution rules.

use time::macros::datetime;
use timelex_attribution::{
    attribute, AliasKind, AliasRecord, ClientRecord, Decision, Directory, DirectoryError, Evidence, History,
    MatterRecord, MatterStatus, ReviewReason,
};
use timelex_types::{ActivityEvent, Actor, Privacy, Source, SourceSystem, Subject};

fn directory() -> Directory {
    Directory::new(
        vec![
            ClientRecord::new("cl_a", Some("a.example")),
            ClientRecord::new("cl_b", Some("b.example")),
            ClientRecord::new("cl_closed", Some("closed.example")),
            ClientRecord::new("cl_solo", Some("solo.example")),
        ],
        vec![
            matter("mt_1", "cl_a", "M-2026-0101", MatterStatus::Open),
            matter("mt_2", "cl_b", "M-2026-0102", MatterStatus::Open),
            matter("mt_3", "cl_b", "M-2026-0103", MatterStatus::Open),
            matter("mt_4", "cl_closed", "M-2026-0104", MatterStatus::Closed),
            matter("mt_5", "cl_solo", "M-2026-0105", MatterStatus::Open),
        ],
        vec![
            alias(AliasKind::MatterReference, "OLD-1", Some("mt_1"), None),
            alias(AliasKind::ClientDomain, "a-alias.example", None, Some("cl_a")),
        ],
    )
    .expect("the fixture directory is valid")
}

fn matter(id: &str, client_id: &str, reference: &str, status: MatterStatus) -> MatterRecord {
    MatterRecord {
        matter_id: id.into(),
        client_id: client_id.into(),
        reference: reference.into(),
        status,
    }
}

fn alias(kind: AliasKind, value: &str, matter_id: Option<&str>, client_id: Option<&str>) -> AliasRecord {
    AliasRecord {
        kind,
        value: value.into(),
        matter_id: matter_id.map(str::to_owned),
        client_id: client_id.map(str::to_owned),
    }
}

fn event(
    id: &str,
    title: Option<&str>,
    thread: Option<&str>,
    document: Option<&str>,
    domains: &[&str],
) -> ActivityEvent {
    ActivityEvent {
        schema_version: "1.0".into(),
        id: id.into(),
        tenant_id: "tn_1".into(),
        source: Source {
            system: SourceSystem::Manual,
            source_id: format!("src_{id}"),
        },
        kind: timelex_types::ActivityKind::ManualEntry,
        actor: Actor {
            user_id: "us_1".into(),
        },
        started_at: datetime!(2026-09-07 10:00 UTC),
        ended_at: datetime!(2026-09-07 10:30 UTC),
        captured_at: datetime!(2026-09-07 10:31 UTC),
        subject: Some(Subject {
            title: title.map(str::to_owned),
            thread_ref: thread.map(str::to_owned),
            document_ref: document.map(str::to_owned),
            participants: domains
                .iter()
                .map(|domain| timelex_types::Participant {
                    role: timelex_types::ParticipantRole::To,
                    domain: (*domain).into(),
                    address: None,
                })
                .collect(),
        }),
        content_ref: None,
        privacy: Privacy {
            content_captured: false,
            redaction_applied: false,
        },
    }
}

fn assigned(decision: &Decision) -> (&str, &str, Evidence) {
    match decision {
        Decision::Assigned {
            client_id,
            matter_id,
            evidence,
        } => (client_id, matter_id, *evidence),
        other => panic!("expected an assignment, got {other:?}"),
    }
}

fn review(decision: &Decision) -> (ReviewReason, Option<&str>, Vec<&str>) {
    match decision {
        Decision::Review {
            reason,
            client_id,
            candidate_matter_ids,
        } => (
            *reason,
            client_id.as_deref(),
            candidate_matter_ids.iter().map(String::as_str).collect(),
        ),
        other => panic!("expected a review, got {other:?}"),
    }
}

#[test]
fn a_matter_reference_in_a_subject_is_assigned() {
    let events = [event(
        "e1",
        Some("[M-2026-0101] Shareholder agreement review"),
        None,
        None,
        &[],
    )];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::MatterReference));
}

#[test]
fn reference_matching_is_case_insensitive_and_ignores_surrounding_punctuation() {
    let events = [event("e1", Some("re: m-2026-0101, draft"), None, None, &[])];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(decision, {
        Decision::Assigned {
            client_id: "cl_a".into(),
            matter_id: "mt_1".into(),
            evidence: Evidence::MatterReference,
        }
    });
}

#[test]
fn a_reference_inside_a_longer_token_is_not_a_hard_key() {
    for title in ["M-2026-01010 draft", "XM-2026-0101 draft", "M-2026-0101A"] {
        let events = [event("e1", Some(title), None, None, &[])];
        let decision = attribute(&events, &directory(), &History::new());
        assert_eq!(
            decision,
            Decision::Unattributed,
            "title {title:?} must not match M-2026-0101"
        );
    }
}

#[test]
fn an_explicit_reference_alias_resolves_the_matter() {
    let events = [event("e1", Some("OLD-1: notes"), None, None, &[])];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::MatterReference));
}

#[test]
fn two_different_references_in_one_group_are_reviewed() {
    let events = [
        event("e1", Some("[M-2026-0101] first"), Some("th_1"), None, &[]),
        event("e2", Some("[M-2026-0102] second"), Some("th_1"), None, &[]),
    ];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, _, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ConflictingHardKeys);
    assert_eq!(candidates, ["mt_1", "mt_2"]);
}

#[test]
fn repeated_references_to_the_same_matter_are_one_hard_key() {
    let events = [
        event("e1", Some("[M-2026-0101] first"), Some("th_1"), None, &[]),
        event("e2", Some("[M-2026-0101] second"), Some("th_1"), None, &[]),
    ];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::MatterReference));
}

#[test]
fn a_reference_to_a_closed_matter_is_reviewed_with_that_matter() {
    let events = [event("e1", Some("[M-2026-0104] legacy"), None, None, &[])];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, client, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ClosedMatter);
    assert_eq!(client, Some("cl_closed"));
    assert_eq!(candidates, ["mt_4"]);
}

#[test]
fn a_reference_that_disagrees_with_the_participant_domain_is_reviewed() {
    let events = [event(
        "e1",
        Some("[M-2026-0101] shareholder agreement"),
        None,
        None,
        &["b.example"],
    )];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, _, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ConflictingHardKeys);
    assert!(candidates.contains(&"mt_1"));
}

#[test]
fn a_thread_carries_its_reference_to_events_that_lack_one() {
    let events = [
        event("e1", Some("[M-2026-0101] first"), Some("th_1"), None, &[]),
        event("e2", None, Some("th_1"), None, &[]),
        event("e3", Some("reply"), Some("th_1"), None, &[]),
    ];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::MatterReference));
}

#[test]
fn a_counterparty_group_needs_the_reference_on_every_event() {
    let events = [
        event("e1", Some("[M-2026-0102] lease"), None, None, &["b.example"]),
        event("e2", Some("call notes"), None, None, &["b.example"]),
    ];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, client, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::MixedEvidence);
    assert_eq!(client, Some("cl_b"));
    assert_eq!(candidates, ["mt_2", "mt_3"]);
}

#[test]
fn a_single_client_domain_with_one_open_matter_is_assigned() {
    let events = [event("e1", Some("call"), None, None, &["a.example"])];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::ClientDomain));
}

#[test]
fn a_client_domain_alias_resolves_the_client() {
    let events = [event("e1", Some("call"), None, None, &["A-ALIAS.EXAMPLE."])];
    let decision = attribute(&events, &directory(), &History::new());
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::ClientDomain));
}

#[test]
fn a_single_client_domain_with_several_open_matters_is_ambiguous() {
    let events = [event("e1", Some("call"), None, None, &["b.example"])];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, client, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::AmbiguousClient);
    assert_eq!(client, Some("cl_b"));
    assert_eq!(candidates, ["mt_2", "mt_3"]);
}

#[test]
fn two_client_domains_in_one_group_are_reviewed() {
    let events = [event(
        "e1",
        Some("joint call"),
        None,
        None,
        &["a.example", "b.example"],
    )];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, _, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::MultipleClients);
    assert_eq!(candidates, ["mt_1", "mt_2", "mt_3"]);
}

#[test]
fn a_domain_whose_client_has_only_closed_matters_is_reviewed() {
    let events = [event("e1", Some("call"), None, None, &["closed.example"])];
    let decision = attribute(&events, &directory(), &History::new());
    let (reason, client, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ClosedMatter);
    assert_eq!(client, Some("cl_closed"));
    assert_eq!(candidates, ["mt_4"]);
}

#[test]
fn a_thread_with_history_assigns_the_remembered_matter() {
    let evidence = [event("e0", Some("first"), Some("th_1"), None, &[])];
    let mut history = History::new();
    history.record(
        &evidence,
        &Decision::Assigned {
            client_id: "cl_b".into(),
            matter_id: "mt_2".into(),
            evidence: Evidence::MatterReference,
        },
    );

    let events = [event("e1", Some("follow up"), Some("th_1"), None, &[])];
    let decision = attribute(&events, &directory(), &history);
    assert_eq!(assigned(&decision), ("cl_b", "mt_2", Evidence::HistoryThread));
}

#[test]
fn a_document_with_history_assigns_the_remembered_matter() {
    let mut history = History::new();
    history.record(
        &[event("e0", None, None, Some("doc_1"), &[])],
        &Decision::Assigned {
            client_id: "cl_b".into(),
            matter_id: "mt_3".into(),
            evidence: Evidence::MatterReference,
        },
    );

    let events = [event("e1", None, None, Some("doc_1"), &[])];
    let decision = attribute(&events, &directory(), &history);
    assert_eq!(assigned(&decision), ("cl_b", "mt_3", Evidence::HistoryDocument));
}

#[test]
fn participant_history_resolves_a_client() {
    let mut history = History::new();
    history.record(
        &[event(
            "e0",
            None,
            Some("th_0"),
            None,
            &["firm.example", "contact.example"],
        )],
        &Decision::Assigned {
            client_id: "cl_a".into(),
            matter_id: "mt_1".into(),
            evidence: Evidence::MatterReference,
        },
    );

    let events = [event(
        "e1",
        Some("call"),
        None,
        None,
        &["contact.example", "firm.example"],
    )];
    let decision = attribute(&events, &directory(), &history);
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::HistoryClient));
}

#[test]
fn conflicting_thread_history_is_reviewed() {
    let mut history = History::new();
    let first = [event("e0", Some("first"), Some("th_1"), None, &[])];
    let second = [event("e0b", Some("second"), Some("th_1"), None, &[])];
    for (events, client, matter) in [(&first[..], "cl_a", "mt_1"), (&second[..], "cl_b", "mt_2")] {
        history.record(
            events,
            &Decision::Assigned {
                client_id: client.into(),
                matter_id: matter.into(),
                evidence: Evidence::MatterReference,
            },
        );
    }

    let events = [event("e1", Some("third"), Some("th_1"), None, &[])];
    let decision = attribute(&events, &directory(), &history);
    let (reason, _, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ConflictingHistory);
    assert_eq!(candidates, ["mt_1", "mt_2"]);
}

#[test]
fn history_does_not_override_a_hard_key() {
    let mut history = History::new();
    history.record(
        &[event("e0", None, Some("th_1"), None, &[])],
        &Decision::Assigned {
            client_id: "cl_b".into(),
            matter_id: "mt_2".into(),
            evidence: Evidence::HistoryThread,
        },
    );

    let events = [event("e1", Some("[M-2026-0101] draft"), Some("th_1"), None, &[])];
    let decision = attribute(&events, &directory(), &history);
    assert_eq!(assigned(&decision), ("cl_a", "mt_1", Evidence::MatterReference));
}

#[test]
fn history_of_a_closed_matter_is_reviewed() {
    let mut history = History::new();
    history.record(
        &[event("e0", None, Some("th_1"), None, &[])],
        &Decision::Assigned {
            client_id: "cl_closed".into(),
            matter_id: "mt_4".into(),
            evidence: Evidence::HistoryThread,
        },
    );

    let events = [event("e1", Some("follow up"), Some("th_1"), None, &[])];
    let decision = attribute(&events, &directory(), &history);
    let (reason, client, candidates) = review(&decision);
    assert_eq!(reason, ReviewReason::ClosedMatter);
    assert_eq!(client, Some("cl_closed"));
    assert_eq!(candidates, ["mt_4"]);
}

#[test]
fn history_records_assignments_only() {
    let events = [event("e0", Some("call"), Some("th_1"), None, &[])];
    let mut history = History::new();
    history.record(
        &events,
        &Decision::Review {
            reason: ReviewReason::AmbiguousClient,
            client_id: Some("cl_b".into()),
            candidate_matter_ids: vec!["mt_2".into(), "mt_3".into()],
        },
    );
    assert!(history.is_empty());

    let decision = attribute(&events, &directory(), &history);
    assert_eq!(decision, Decision::Unattributed);
}

#[test]
fn an_event_without_evidence_is_unattributed() {
    let events = [event("e1", Some("focus"), None, None, &[])];
    assert_eq!(
        attribute(&events, &directory(), &History::new()),
        Decision::Unattributed
    );
}

#[test]
fn the_directory_refuses_an_ambiguous_reference() {
    let error = Directory::new(
        vec![ClientRecord::new("cl_a", None)],
        vec![
            matter("mt_1", "cl_a", "M-1", MatterStatus::Open),
            matter("mt_2", "cl_a", "m-1 ", MatterStatus::Open),
        ],
        vec![],
    )
    .unwrap_err();
    assert_eq!(error, DirectoryError::DuplicateReference("m-1".into()));
}

#[test]
fn the_directory_refuses_an_alias_without_a_target() {
    let error = Directory::new(
        vec![ClientRecord::new("cl_a", None)],
        vec![matter("mt_1", "cl_a", "M-1", MatterStatus::Open)],
        vec![alias(AliasKind::MatterReference, "OLD", None, None)],
    )
    .unwrap_err();
    assert_eq!(error, DirectoryError::AliasTargetMissing);
}

#[test]
fn the_directory_refuses_an_alias_to_an_unknown_matter() {
    let error = Directory::new(
        vec![ClientRecord::new("cl_a", None)],
        vec![matter("mt_1", "cl_a", "M-1", MatterStatus::Open)],
        vec![alias(AliasKind::MatterReference, "OLD", Some("mt_9"), None)],
    )
    .unwrap_err();
    assert_eq!(error, DirectoryError::AliasTargetMissing);
}

#[test]
fn the_directory_refuses_two_clients_claiming_one_domain() {
    let error = Directory::new(
        vec![
            ClientRecord::new("cl_a", Some("shared.example")),
            ClientRecord::new("cl_b", Some("SHARED.EXAMPLE.")),
        ],
        vec![],
        vec![],
    )
    .unwrap_err();
    assert_eq!(error, DirectoryError::DuplicateDomain("shared.example".into()));
}

#[test]
fn the_directory_refuses_an_empty_alias() {
    let error = Directory::new(
        vec![ClientRecord::new("cl_a", None)],
        vec![matter("mt_1", "cl_a", "M-1", MatterStatus::Open)],
        vec![alias(AliasKind::MatterReference, "  ", Some("mt_1"), None)],
    )
    .unwrap_err();
    assert_eq!(
        error,
        DirectoryError::EmptyAlias {
            kind: AliasKind::MatterReference
        }
    );
}

#[test]
fn the_directory_allows_one_value_under_different_kinds() {
    Directory::new(
        vec![ClientRecord::new("cl_a", None), ClientRecord::new("cl_b", None)],
        vec![matter("mt_1", "cl_a", "M-1", MatterStatus::Open)],
        vec![
            alias(AliasKind::MatterReference, "OLD", Some("mt_1"), None),
            alias(AliasKind::ClientDomain, "old", None, Some("cl_b")),
        ],
    )
    .expect("kind and value together identify an alias");
}

#[test]
fn the_directory_refuses_one_alias_value_naming_two_targets() {
    let error = Directory::new(
        vec![ClientRecord::new("cl_a", None), ClientRecord::new("cl_b", None)],
        vec![
            matter("mt_1", "cl_a", "M-1", MatterStatus::Open),
            matter("mt_2", "cl_b", "M-2", MatterStatus::Open),
        ],
        vec![
            alias(AliasKind::MatterReference, "OLD", Some("mt_1"), None),
            alias(AliasKind::MatterReference, "old ", Some("mt_2"), None),
        ],
    )
    .unwrap_err();
    assert_eq!(error, DirectoryError::DuplicateReference("old".into()));
}
