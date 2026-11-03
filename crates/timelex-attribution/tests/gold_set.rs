// Written by Kone Tshivhinda & Claude
//! Measures stages 0 and 1 against the committed synthetic gold set.
//!
//! The corpus is the first labelled input to attribution. This test segments the stored events per
//! actor, attributes every block in time order with one firm-wide history, and checks three things:
//! no assignment contradicts a label, every signal that a hard key can resolve is either assigned
//! the right matter or offered it to a reviewer, and no review decision loses the correct candidate.
//! The printed report is the measurement recorded in the ADR and WORKLOG.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use time::OffsetDateTime;
use timelex_attribution::{
    attribute, ClientRecord, Decision, Directory, Evidence, History, MatterRecord, MatterStatus,
};
use timelex_segmentation::{segment, SegmentationConfig};
use timelex_types::ActivityEvent;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../eval/synthetic/corpus");

#[derive(Deserialize)]
struct EventsFile {
    events: Vec<ActivityEvent>,
}

#[derive(Deserialize)]
struct LabelsFile {
    labels: Vec<Label>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Label {
    event_id: String,
    client_id: Option<String>,
    matter_id: Option<String>,
    expected_outcome: String,
    expected_storage: String,
    evidence: String,
}

#[derive(Deserialize)]
struct FirmFile {
    clients: Vec<FirmClient>,
    matters: Vec<FirmMatter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirmClient {
    client_id: String,
    domain: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirmMatter {
    matter_id: String,
    client_id: String,
    reference: String,
    status: String,
}

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = format!("{CORPUS}/{name}");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

fn directory(firm: &FirmFile) -> Directory {
    let clients = firm
        .clients
        .iter()
        .map(|client| ClientRecord::new(&client.client_id, client.domain.as_deref()))
        .collect();
    let matters = firm
        .matters
        .iter()
        .map(|matter| MatterRecord {
            matter_id: matter.matter_id.clone(),
            client_id: matter.client_id.clone(),
            reference: matter.reference.clone(),
            status: match matter.status.as_str() {
                "open" => MatterStatus::Open,
                "closed" => MatterStatus::Closed,
                other => panic!("unknown matter status {other:?}"),
            },
        })
        .collect();
    Directory::new(clients, matters, vec![]).expect("the gold-set firm is a valid directory")
}

#[derive(Default)]
struct Report {
    blocks: usize,
    assigned_blocks: usize,
    review_blocks: usize,
    unattributed_blocks: usize,
    assigned_events: usize,
    correct_events: usize,
    wrong_matter_events: usize,
    false_positive_events: usize,
    auto_post_events: usize,
    auto_post_covered: usize,
    auto_post_without_block: usize,
    represented_events: usize,
    review_events: usize,
    review_candidate_misses: usize,
    review_skipped: usize,
    deferred_content_events: usize,
    by_evidence: BTreeMap<&'static str, usize>,
}

#[test]
fn stages_zero_and_one_are_measured_against_the_gold_set() {
    let events: EventsFile = load("events.json");
    let labels: LabelsFile = load("labels.json");
    let firm: FirmFile = load("firm.json");
    let directory = directory(&firm);

    let label_by_id: BTreeMap<&str, &Label> = labels
        .labels
        .iter()
        .map(|label| (label.event_id.as_str(), label))
        .collect();
    assert_eq!(label_by_id.len(), events.events.len(), "one label per event");

    let stored: Vec<ActivityEvent> = events
        .events
        .into_iter()
        .filter(|event| {
            label_by_id
                .get(event.id.as_str())
                .is_some_and(|label| label.expected_storage == "stored")
        })
        .collect();

    let mut by_actor: BTreeMap<&str, Vec<ActivityEvent>> = BTreeMap::new();
    for event in &stored {
        by_actor
            .entry(event.actor.user_id.as_str())
            .or_default()
            .push(event.clone());
    }
    let event_by_id: BTreeMap<&str, &ActivityEvent> =
        stored.iter().map(|event| (event.id.as_str(), event)).collect();

    struct Block {
        started_at: OffsetDateTime,
        events: Vec<ActivityEvent>,
    }
    let mut blocks = Vec::new();
    for events in by_actor.values() {
        for block in segment(events, &SegmentationConfig::default()).expect("the corpus segments") {
            let block_events = block
                .event_ids
                .iter()
                .map(|id| (*event_by_id.get(id.as_str()).expect("block cites a stored event")).clone())
                .collect();
            blocks.push(Block {
                started_at: block.started_at,
                events: block_events,
            });
        }
    }
    blocks.sort_by(|left, right| {
        (left.started_at, left.events.len(), &left.events[0].id).cmp(&(
            right.started_at,
            right.events.len(),
            &right.events[0].id,
        ))
    });

    let mut history = History::new();
    let mut report = Report::default();
    let mut decision_by_event: BTreeMap<String, Decision> = BTreeMap::new();
    let mut represented: BTreeSet<&str> = BTreeSet::new();

    for block in &blocks {
        report.blocks += 1;
        let decision = attribute(&block.events, &directory, &history);
        match &decision {
            Decision::Assigned { evidence, .. } => {
                report.assigned_blocks += 1;
                *report.by_evidence.entry(evidence_name(*evidence)).or_default() += 1;
            }
            Decision::Review { .. } => report.review_blocks += 1,
            Decision::Unattributed => report.unattributed_blocks += 1,
        }
        history.record(&block.events, &decision);
        for event in &block.events {
            decision_by_event.insert(event.id.clone(), decision.clone());
            represented.insert(event.id.as_str());
        }

        for event in &block.events {
            let label = label_by_id[event.id.as_str()];
            if label.matter_id.is_none() && matches!(decision, Decision::Assigned { .. }) {
                report.false_positive_events += 1;
            }
        }

        match decision {
            Decision::Assigned {
                ref client_id,
                ref matter_id,
                ..
            } => {
                for event in &block.events {
                    let label = label_by_id[event.id.as_str()];
                    report.assigned_events += 1;
                    if label.expected_outcome == "review" {
                        report.review_skipped += 1;
                    }
                    let client_ok = label.client_id.as_deref().is_none_or(|id| id == client_id);
                    let matter_ok = label.matter_id.as_deref().is_none_or(|id| id == matter_id);
                    if client_ok && matter_ok && label.matter_id.is_some() {
                        report.correct_events += 1;
                    } else if label.matter_id.is_some() {
                        report.wrong_matter_events += 1;
                    }
                }
            }
            Decision::Review {
                ref candidate_matter_ids,
                ..
            } => {
                for event in &block.events {
                    let label = label_by_id[event.id.as_str()];
                    if let Some(matter) = &label.matter_id {
                        if !candidate_matter_ids.contains(matter) {
                            report.review_candidate_misses += 1;
                        }
                    }
                }
            }
            Decision::Unattributed => {}
        }
    }

    for label in &labels.labels {
        if label.expected_outcome == "auto_post" {
            report.auto_post_events += 1;
            if !represented.contains(label.event_id.as_str()) {
                report.auto_post_without_block += 1;
                continue;
            }
            if let Some(decision) = decision_by_event.get(&label.event_id) {
                let covered = match decision {
                    Decision::Assigned { matter_id, .. } => label.matter_id.as_deref() == Some(matter_id),
                    Decision::Review {
                        candidate_matter_ids, ..
                    } => candidate_matter_ids
                        .contains(label.matter_id.as_ref().expect("auto_post names a matter")),
                    Decision::Unattributed => false,
                };
                if covered {
                    report.auto_post_covered += 1;
                }
            }
        }
        if label.expected_outcome == "review"
            && matches!(label.evidence.as_str(), "hard_key" | "counterparty")
        {
            report.review_events += 1;
        }
        if label.evidence == "content" {
            report.deferred_content_events += 1;
        }
    }

    report.represented_events = represented.len();

    println!(
        "attribution: {} blocks ({} assigned, {} review, {} unattributed)",
        report.blocks, report.assigned_blocks, report.review_blocks, report.unattributed_blocks
    );
    println!(
        "represented events: {} of {} stored ({} claimed no time)",
        report.represented_events,
        stored.len(),
        stored.len() - report.represented_events
    );
    println!(
        "assigned events: {} of {} correct, {} wrong matter, {} false positive",
        report.correct_events,
        report.assigned_events,
        report.wrong_matter_events,
        report.false_positive_events
    );
    println!(
        "auto-post signals: {}/{} assigned the right matter or offered to review; {} claimed no time",
        report.auto_post_covered, report.auto_post_events, report.auto_post_without_block
    );
    println!(
        "review signals: {}, candidate misses: {}, wrongly auto-posted: {}",
        report.review_events, report.review_candidate_misses, report.review_skipped
    );
    println!(
        "content-dependent signals deferred to later stages: {}",
        report.deferred_content_events
    );
    println!("assignments by evidence: {:?}", report.by_evidence);

    assert!(
        report.represented_events <= stored.len(),
        "a block cannot cite more events than were stored"
    );
    assert_eq!(
        report.auto_post_covered + report.auto_post_without_block,
        report.auto_post_events,
        "an auto-post signal is either represented by a block or claimed no time"
    );
    assert!(
        report.assigned_events > 0,
        "the corpus must exercise an assignment"
    );
    assert_eq!(
        report.wrong_matter_events, 0,
        "no assignment may contradict a label"
    );
    assert_eq!(
        report.false_positive_events, 0,
        "no unassigned work may name a matter"
    );
    assert_eq!(
        report.correct_events, report.assigned_events,
        "every assigned event must be correct: precision is 100%"
    );
    assert_eq!(
        report.review_candidate_misses, 0,
        "a review must name the correct matter"
    );
    assert_eq!(
        report.review_skipped, 0,
        "a signal labelled review must not be auto-posted"
    );
}

fn evidence_name(evidence: Evidence) -> &'static str {
    match evidence {
        Evidence::MatterReference => "matter_reference",
        Evidence::ClientDomain => "client_domain",
        Evidence::HistoryThread => "history_thread",
        Evidence::HistoryDocument => "history_document",
        Evidence::HistoryClient => "history_client",
    }
}
