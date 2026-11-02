// Written by Kone Tshivhinda & Claude
//! Behaviour and property tests for deterministic segmentation.

use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use timelex_segmentation::{
    segment, GroupKey, SegmentationConfig, SegmentationError, TimeBlock, TimeBlockClass,
};
use timelex_types::{
    ActivityEvent, ActivityKind, Actor, Participant, ParticipantRole, Privacy, Source, SourceSystem, Subject,
};

fn t(seconds: i64) -> OffsetDateTime {
    datetime!(2026-09-07 00:00 UTC) + Duration::seconds(seconds)
}

fn at(hours: i64, minutes: i64) -> i64 {
    hours * 3600 + minutes * 60
}

fn event(id: &str, kind: ActivityKind, start: i64, end: i64) -> ActivityEvent {
    ActivityEvent {
        schema_version: "1.0".to_owned(),
        id: id.to_owned(),
        tenant_id: "tn_test".to_owned(),
        source: Source {
            system: SourceSystem::DesktopAgent,
            source_id: format!("src_{id}"),
        },
        kind,
        actor: Actor {
            user_id: "us_test".to_owned(),
        },
        started_at: t(start),
        ended_at: t(end),
        captured_at: t(end) + Duration::minutes(1),
        subject: None,
        content_ref: None,
        privacy: Privacy {
            content_captured: false,
            redaction_applied: false,
        },
    }
}

fn threaded(mut event: ActivityEvent, thread: &str) -> ActivityEvent {
    let subject = event.subject.get_or_insert_with(Subject::default);
    subject.thread_ref = Some(thread.to_owned());
    event
}

fn documented(mut event: ActivityEvent, document: &str) -> ActivityEvent {
    let subject = event.subject.get_or_insert_with(Subject::default);
    subject.document_ref = Some(document.to_owned());
    event
}

fn with_domains(mut event: ActivityEvent, domains: &[&str]) -> ActivityEvent {
    let subject = event.subject.get_or_insert_with(Subject::default);
    subject.participants = domains
        .iter()
        .map(|domain| Participant {
            role: ParticipantRole::Attendee,
            domain: (*domain).to_owned(),
            address: None,
        })
        .collect();
    event
}

fn doc_event(id: &str, start: i64, end: i64) -> ActivityEvent {
    event(id, ActivityKind::DocumentEdit, start, end)
}

fn run(events: &[ActivityEvent]) -> Vec<TimeBlock> {
    segment(events, &SegmentationConfig::default()).expect("fixture is valid")
}

fn union_seconds(events: &[ActivityEvent]) -> u64 {
    let mut intervals: Vec<(i64, i64)> = events
        .iter()
        .filter_map(|event| {
            let start = event.started_at.unix_timestamp();
            let end = event.ended_at.unix_timestamp();
            (end > start).then_some((start, end))
        })
        .collect();
    intervals.sort_unstable();
    let mut total = 0u64;
    let mut current: Option<(i64, i64)> = None;
    for (start, end) in intervals {
        match current {
            Some((cur_start, cur_end)) if start <= cur_end => {
                current = Some((cur_start, cur_end.max(end)));
            }
            Some((cur_start, cur_end)) => {
                total += (cur_end - cur_start) as u64;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((start, end)) = current {
        total += (end - start) as u64;
    }
    total
}

fn assert_disjoint(blocks: &[TimeBlock]) {
    for pair in blocks.windows(2) {
        assert!(
            pair[0].ended_at <= pair[1].started_at,
            "blocks overlap: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn empty_input_yields_no_blocks() {
    assert_eq!(run(&[]), Vec::new());
}

#[test]
fn single_event_becomes_one_block() {
    let blocks = run(&[threaded(doc_event("ae_1", at(9, 0), at(10, 0)), "th_1")]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].started_at, t(at(9, 0)));
    assert_eq!(blocks[0].ended_at, t(at(10, 0)));
    assert_eq!(blocks[0].duration_seconds, 3600);
    assert_eq!(blocks[0].class, TimeBlockClass::Passive);
    assert_eq!(blocks[0].group, GroupKey::Thread("th_1".to_owned()));
    assert_eq!(blocks[0].event_ids, vec!["ae_1".to_owned()]);
}

#[test]
fn same_thread_within_gap_merges() {
    let blocks = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"),
        threaded(doc_event("ae_2", at(9, 34), at(10, 0)), "th_1"),
    ]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].duration_seconds, 3600);
    assert_eq!(blocks[0].event_ids, vec!["ae_1".to_owned(), "ae_2".to_owned()]);
}

#[test]
fn same_thread_beyond_gap_stays_apart() {
    let blocks = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"),
        threaded(doc_event("ae_2", at(9, 36), at(10, 0)), "th_1"),
    ]);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].duration_seconds, 1800);
    assert_eq!(blocks[1].duration_seconds, 1440);
}

#[test]
fn gap_boundary_is_inclusive() {
    let exact = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"),
        threaded(doc_event("ae_2", at(9, 35), at(10, 0)), "th_1"),
    ]);
    assert_eq!(exact.len(), 1);

    let over = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"),
        threaded(doc_event("ae_2", at(9, 35) + 1, at(10, 0)), "th_1"),
    ]);
    assert_eq!(over.len(), 2);
}

#[test]
fn different_threads_never_merge() {
    let blocks = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"),
        threaded(doc_event("ae_2", at(9, 30), at(10, 0)), "th_2"),
    ]);
    assert_eq!(blocks.len(), 2);
}

#[test]
fn overlapping_events_in_one_group_merge_to_their_union() {
    let blocks = run(&[
        threaded(doc_event("ae_1", at(9, 0), at(10, 0)), "th_1"),
        threaded(doc_event("ae_2", at(9, 30), at(9, 45)), "th_1"),
    ]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].duration_seconds, 3600);
}

#[test]
fn thread_grouping_outranks_document_and_counterparty() {
    let first = with_domains(
        documented(threaded(doc_event("ae_1", at(9, 0), at(9, 30)), "th_1"), "doc_a"),
        &["one.example"],
    );
    let second = with_domains(
        documented(threaded(doc_event("ae_2", at(9, 31), at(10, 0)), "th_1"), "doc_b"),
        &["two.example"],
    );
    let blocks = run(&[first, second]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].group, GroupKey::Thread("th_1".to_owned()));
}

#[test]
fn document_grouping_applies_without_a_thread() {
    let blocks = run(&[
        documented(doc_event("ae_1", at(9, 0), at(9, 30)), "doc_a"),
        documented(doc_event("ae_2", at(9, 32), at(10, 0)), "doc_a"),
    ]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].group, GroupKey::Document("doc_a".to_owned()));
}

#[test]
fn counterparty_domains_group_case_insensitively_and_ignore_order() {
    let first = with_domains(
        doc_event("ae_1", at(9, 0), at(9, 30)),
        &["B.Example", "a.example"],
    );
    let second = with_domains(
        doc_event("ae_2", at(9, 32), at(10, 0)),
        &["a.example.", "b.example"],
    );
    let blocks = run(&[first, second]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0].group,
        GroupKey::Counterparty(vec!["a.example".to_owned(), "b.example".to_owned()])
    );
}

#[test]
fn events_without_a_shared_reference_stay_alone() {
    let blocks = run(&[
        doc_event("ae_1", at(9, 0), at(9, 30)),
        doc_event("ae_2", at(9, 31), at(10, 0)),
    ]);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].group, GroupKey::Event("ae_1".to_owned()));
    assert_eq!(blocks[1].group, GroupKey::Event("ae_2".to_owned()));
}

#[test]
fn a_call_splits_passive_work_around_it() {
    let blocks = run(&[
        threaded(doc_event("ae_doc", at(9, 0), at(10, 0)), "th_doc"),
        threaded(doc_event("ae_view", at(9, 50), at(9, 55)), "th_doc"),
        threaded(
            event("ae_call", ActivityKind::Call, at(9, 20), at(9, 30)),
            "th_call",
        ),
    ]);
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        (
            blocks[0].started_at,
            blocks[0].ended_at,
            blocks[0].duration_seconds
        ),
        (t(at(9, 0)), t(at(9, 20)), 1200)
    );
    assert_eq!(blocks[0].class, TimeBlockClass::Passive);
    assert_eq!(blocks[0].event_ids, vec!["ae_doc".to_owned()]);
    assert_eq!(
        (
            blocks[1].started_at,
            blocks[1].ended_at,
            blocks[1].duration_seconds
        ),
        (t(at(9, 20)), t(at(9, 30)), 600)
    );
    assert_eq!(blocks[1].class, TimeBlockClass::Anchor);
    assert_eq!(blocks[1].event_ids, vec!["ae_call".to_owned()]);
    assert_eq!(
        (
            blocks[2].started_at,
            blocks[2].ended_at,
            blocks[2].duration_seconds
        ),
        (t(at(9, 30)), t(at(10, 0)), 1800)
    );
    assert_eq!(
        blocks[2].event_ids,
        vec!["ae_doc".to_owned(), "ae_view".to_owned()]
    );
    assert_disjoint(&blocks);
}

#[test]
fn passive_work_fully_covered_by_an_anchor_produces_no_block() {
    let blocks = run(&[
        threaded(doc_event("ae_doc", at(9, 0), at(9, 5)), "th_doc"),
        threaded(
            event("ae_call", ActivityKind::Call, at(9, 0), at(9, 10)),
            "th_call",
        ),
    ]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].class, TimeBlockClass::Anchor);
    assert_eq!(blocks[0].duration_seconds, 600);
}

#[test]
fn overlapping_anchors_are_trimmed_not_double_counted() {
    let blocks = run(&[
        threaded(event("ae_a", ActivityKind::Call, at(9, 0), at(9, 30)), "th_a"),
        threaded(event("ae_b", ActivityKind::Call, at(9, 15), at(9, 45)), "th_b"),
    ]);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].duration_seconds, 1800);
    assert_eq!(blocks[1].duration_seconds, 900);
    assert_eq!(blocks[1].started_at, t(at(9, 30)));
    assert_eq!(blocks[1].event_ids, vec!["ae_b".to_owned()]);
    assert_eq!(blocks[0].duration_seconds + blocks[1].duration_seconds, 2700);
}

#[test]
fn zero_length_events_produce_no_block() {
    let blocks = run(&[
        threaded(doc_event("ae_empty", at(9, 0), at(9, 0)), "th_1"),
        threaded(doc_event("ae_real", at(9, 5), at(9, 10)), "th_1"),
    ]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].event_ids, vec!["ae_real".to_owned()]);
}

#[test]
fn sub_second_intervals_round_outward() {
    let mut sample = doc_event("ae_1", 0, 1);
    sample.started_at = t(0) + Duration::milliseconds(400);
    sample.ended_at = t(0) + Duration::milliseconds(1200);
    sample.captured_at = t(0) + Duration::minutes(1);
    let blocks = run(&[sample]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].started_at, t(0));
    assert_eq!(blocks[0].ended_at, t(2));
    assert_eq!(blocks[0].duration_seconds, 2);
}

#[test]
fn every_activity_kind_maps_to_the_expected_class() {
    let anchors = [
        ActivityKind::Meeting,
        ActivityKind::Call,
        ActivityKind::ManualEntry,
    ];
    let passive = [
        ActivityKind::EmailSent,
        ActivityKind::EmailReceived,
        ActivityKind::DocumentEdit,
        ActivityKind::DocumentView,
        ActivityKind::WebResearch,
        ActivityKind::DesktopFocus,
    ];
    for (index, kind) in anchors.into_iter().enumerate() {
        let blocks = run(&[event(&format!("ae_a{index}"), kind, at(9, 0), at(9, 30))]);
        assert_eq!(blocks[0].class, TimeBlockClass::Anchor, "{kind:?}");
    }
    for (index, kind) in passive.into_iter().enumerate() {
        let blocks = run(&[event(&format!("ae_p{index}"), kind, at(9, 0), at(9, 30))]);
        assert_eq!(blocks[0].class, TimeBlockClass::Passive, "{kind:?}");
    }
}

#[test]
fn end_before_start_is_refused() {
    let error = segment(
        &[threaded(doc_event("ae_1", at(10, 0), at(9, 0)), "th_1")],
        &SegmentationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SegmentationError::EndBeforeStart {
            event_id: "ae_1".to_owned()
        }
    );
}

#[test]
fn negative_gap_tolerance_is_refused() {
    let error = segment(
        &[doc_event("ae_1", at(9, 0), at(9, 30))],
        &SegmentationConfig {
            gap_tolerance_seconds: -1,
        },
    )
    .unwrap_err();
    assert_eq!(error, SegmentationError::NegativeGapTolerance(-1));
}

#[test]
fn mixed_tenants_are_refused() {
    let mut other = doc_event("ae_2", at(9, 30), at(10, 0));
    other.tenant_id = "tn_other".to_owned();
    let error = segment(
        &[doc_event("ae_1", at(9, 0), at(9, 30)), other],
        &Default::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SegmentationError::MixedTenants {
            first: "tn_test".to_owned(),
            found: "tn_other".to_owned()
        }
    );
}

#[test]
fn mixed_actors_are_refused() {
    let mut other = doc_event("ae_2", at(9, 30), at(10, 0));
    other.actor.user_id = "us_other".to_owned();
    let error = segment(
        &[doc_event("ae_1", at(9, 0), at(9, 30)), other],
        &Default::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SegmentationError::MixedActors {
            first: "us_test".to_owned(),
            found: "us_other".to_owned()
        }
    );
}

#[test]
fn input_order_does_not_change_the_output() {
    let events = vec![
        threaded(doc_event("ae_1", at(9, 0), at(9, 40)), "th_a"),
        threaded(doc_event("ae_2", at(9, 42), at(10, 0)), "th_a"),
        threaded(event("ae_3", ActivityKind::Call, at(9, 20), at(9, 30)), "th_b"),
        threaded(doc_event("ae_4", at(11, 0), at(11, 30)), "th_c"),
        event("ae_5", ActivityKind::WebResearch, at(11, 10), at(11, 50)),
        threaded(doc_event("ae_6", at(13, 0), at(14, 0)), "th_d"),
    ];
    let expected = run(&events);
    let mut reversed = events.clone();
    reversed.reverse();
    assert_eq!(run(&reversed), expected);

    let mut state = 0x5eed_u64;
    for _ in 0..50 {
        let mut shuffled = events.clone();
        for index in (1..shuffled.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let swap = ((state >> 33) as usize) % (index + 1);
            shuffled.swap(index, swap);
        }
        assert_eq!(run(&shuffled), expected);
    }
}

#[test]
fn blocks_are_ordered_disjoint_and_cover_the_union_exactly() {
    let mut state = 0x1234_5678_u64;
    let mut events = Vec::new();
    for index in 0..200 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let start = (state >> 20) as i64 % (20 * 3600);
        let length = 60 + ((state >> 8) as i64 % 3600);
        let kind = match index % 7 {
            0 => ActivityKind::Meeting,
            1 => ActivityKind::Call,
            2 => ActivityKind::EmailSent,
            3 => ActivityKind::DocumentEdit,
            4 => ActivityKind::WebResearch,
            5 => ActivityKind::DesktopFocus,
            _ => ActivityKind::ManualEntry,
        };
        let group = index % 9;
        let mut sample = event(&format!("ae_{index:04}"), kind, start, start + length);
        sample = match index % 3 {
            0 => threaded(sample, &format!("th_{group}")),
            1 => documented(sample, &format!("doc_{group}")),
            _ => with_domains(sample, &[&format!("client{group}.example")]),
        };
        events.push(sample);
    }

    let blocks = run(&events);
    assert!(!blocks.is_empty());
    assert_disjoint(&blocks);
    for block in &blocks {
        assert!(block.ended_at > block.started_at);
        assert_eq!(
            block.duration_seconds,
            (block.ended_at - block.started_at).whole_seconds() as u64
        );
        assert!(!block.event_ids.is_empty());
    }
    assert_eq!(
        blocks.iter().map(|block| block.duration_seconds).sum::<u64>(),
        union_seconds(&events)
    );

    let mut seen = std::collections::BTreeSet::new();
    for block in &blocks {
        for second in block.started_at.unix_timestamp()..block.ended_at.unix_timestamp() {
            assert!(seen.insert(second), "second {second} claimed twice");
        }
    }
}

#[test]
fn repeated_runs_are_identical() {
    let events = vec![
        threaded(doc_event("ae_1", at(9, 0), at(9, 40)), "th_a"),
        threaded(event("ae_2", ActivityKind::Call, at(9, 20), at(9, 30)), "th_b"),
        with_domains(doc_event("ae_3", at(9, 35), at(10, 0)), &["a.example"]),
    ];
    assert_eq!(run(&events), run(&events));
}
