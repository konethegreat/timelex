// Written by Kone Tshivhinda & Claude
//! Measures calibration on the committed synthetic gold set.
//!
//! The corpus holds one tenant (tn_gb911f19670423736) and labels without probability scores, so
//! this run measures stages 0 and 1 only: each assigned block contributes its evidence class as a
//! raw score and its label agreement as the outcome. The run prints the reliability diagram, the
//! derived precision floor and what that floor would auto-post. The numbers are recorded in the
//! P3-08 row of docs/TASKS.md and the WORKLOG.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use time::OffsetDateTime;
use timelex_attribution::{
    attribute, ClientRecord, Decision, Directory, Evidence, History, MatterRecord, MatterStatus,
};
use timelex_calibration::{derive_threshold, diagram, reliability, Calibrator, Observation};
use timelex_segmentation::{segment, SegmentationConfig};
use timelex_types::ActivityEvent;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../eval/synthetic/corpus");

/// Precision floor the autonomy ladder needs before a decision may post without review.
const PRECISION_FLOOR: f64 = 0.98;

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
    expected_storage: String,
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

/// Nominal confidence per evidence class, weakest history last (ADR-0029).
fn evidence_confidence(evidence: Evidence) -> f64 {
    match evidence {
        Evidence::MatterReference => 0.95,
        Evidence::ClientDomain => 0.90,
        Evidence::HistoryThread => 0.85,
        Evidence::HistoryDocument => 0.80,
        Evidence::HistoryClient => 0.70,
    }
}

struct Measurement {
    tenant: String,
    observations: Vec<Observation>,
    stored_events: usize,
    labelled_matter_events: usize,
    assigned_blocks: usize,
}

fn measure() -> Measurement {
    let events: EventsFile = load("events.json");
    let labels: LabelsFile = load("labels.json");
    let firm: FirmFile = load("firm.json");
    let directory = directory(&firm);

    let label_by_id: BTreeMap<&str, &Label> = labels
        .labels
        .iter()
        .map(|label| (label.event_id.as_str(), label))
        .collect();

    let stored: Vec<ActivityEvent> = events
        .events
        .into_iter()
        .filter(|event| {
            label_by_id
                .get(event.id.as_str())
                .is_some_and(|label| label.expected_storage == "stored")
        })
        .collect();
    let labelled_matter_events = labels
        .labels
        .iter()
        .filter(|label| label.expected_storage == "stored" && label.matter_id.is_some())
        .count();
    let tenants: BTreeSet<&str> = stored.iter().map(|event| event.tenant_id.as_str()).collect();
    assert_eq!(
        tenants.len(),
        1,
        "a per-tenant calibration reads exactly one tenant"
    );
    let tenant = tenants
        .into_iter()
        .next()
        .expect("the corpus holds stored events")
        .to_owned();

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
    let mut observations = Vec::new();
    let mut assigned_blocks = 0;
    for block in &blocks {
        let decision = attribute(&block.events, &directory, &history);
        history.record(&block.events, &decision);
        let Decision::Assigned {
            client_id,
            matter_id,
            evidence,
        } = decision
        else {
            continue;
        };
        assigned_blocks += 1;
        let raw = evidence_confidence(evidence);
        for event in &block.events {
            let label = label_by_id[event.id.as_str()];
            let correct = label.client_id.as_deref() == Some(client_id.as_str())
                && label.matter_id.as_deref() == Some(matter_id.as_str());
            observations.push(Observation::new(raw, correct).expect("an evidence confidence is in [0, 1]"));
        }
    }

    Measurement {
        tenant,
        observations,
        stored_events: stored.len(),
        labelled_matter_events,
        assigned_blocks,
    }
}

#[test]
fn the_gold_set_yields_a_monotone_map_and_a_threshold_that_holds_the_floor() {
    let measurement = measure();
    assert!(
        !measurement.observations.is_empty(),
        "the corpus must exercise an assignment"
    );
    let correct = measurement
        .observations
        .iter()
        .filter(|observation| observation.correct)
        .count();
    assert!(
        correct <= measurement.labelled_matter_events,
        "an assignment cannot be correct more often than the labels allow: {correct} of {}",
        measurement.labelled_matter_events
    );

    let calibrator = Calibrator::fit(&measurement.observations).expect("the sample fits");
    for pair in calibrator.points().windows(2) {
        assert!(pair[0].probability <= pair[1].probability);
    }

    let raw = reliability(&measurement.observations).expect("the sample measures");
    let calibrated_observations: Vec<Observation> = measurement
        .observations
        .iter()
        .map(|observation| {
            let probability = calibrator
                .calibrate(observation.raw)
                .expect("a fitted score calibrates");
            Observation::new(probability, observation.correct).expect("a calibrated probability is in [0, 1]")
        })
        .collect();
    let calibrated = reliability(&calibrated_observations).expect("the sample measures");
    assert!(
        calibrated.brier <= raw.brier + 1e-12,
        "calibration cannot raise Brier: {} then {}",
        raw.brier,
        calibrated.brier
    );

    let chosen = derive_threshold(&calibrated_observations, PRECISION_FLOOR)
        .expect("the floor is valid")
        .expect("the gold set must support the review-free precision floor");
    assert!(chosen.precision >= PRECISION_FLOOR);
    assert!(chosen.selected > 0);
    assert_eq!(
        chosen.selected,
        calibrated_observations
            .iter()
            .filter(|observation| observation.raw >= chosen.score)
            .count(),
        "the threshold must select exactly the scores at or above it"
    );

    let recall = chosen.correct as f64 / measurement.labelled_matter_events as f64;
    let coverage = chosen.selected as f64 / measurement.stored_events as f64;

    println!(
        "tenant {}: {} stored events, {} with a matter, {} assigned blocks",
        measurement.tenant,
        measurement.stored_events,
        measurement.labelled_matter_events,
        measurement.assigned_blocks,
    );
    println!("{}", diagram(&raw, 24));
    println!("{}", diagram(&calibrated, 24));
    println!(
        "threshold {:.3} (floor {:.2}): {} of {} events selected, {} correct, recall {:.3}, coverage {:.3}",
        chosen.score, PRECISION_FLOOR, chosen.selected, calibrated.count, chosen.correct, recall, coverage
    );
}
