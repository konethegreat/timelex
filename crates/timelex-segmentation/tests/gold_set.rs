// Written by Kone Tshivhinda & Claude
//! Segment the committed synthetic gold set with the real contract types.
//!
//! This is the join between P3-01 and P3-03: the corpus is written as contract events, so a drift
//! in either the event shape or the segmentation rules fails here as well as in the Python check.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;
use timelex_segmentation::{segment, SegmentationConfig, TimeBlock, TimeBlockClass};
use timelex_types::ActivityEvent;

#[derive(Deserialize)]
struct Corpus {
    events: Vec<ActivityEvent>,
}

fn corpus() -> Vec<ActivityEvent> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../eval/synthetic/corpus/events.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str::<Corpus>(&raw)
        .expect("corpus parses as contract events")
        .events
}

fn by_actor(events: &[ActivityEvent]) -> BTreeMap<String, Vec<ActivityEvent>> {
    let mut actors: BTreeMap<String, Vec<ActivityEvent>> = BTreeMap::new();
    for event in events {
        actors
            .entry(event.actor.user_id.clone())
            .or_default()
            .push(event.clone());
    }
    actors
}

fn union_seconds(events: &[ActivityEvent]) -> u64 {
    let mut spans: Vec<(i64, i64)> = events
        .iter()
        .filter_map(|event| {
            let start = event.started_at.unix_timestamp();
            let end = event.ended_at.unix_timestamp();
            (end > start).then_some((start, end))
        })
        .collect();
    spans.sort_unstable();
    let mut total = 0u64;
    let mut current: Option<(i64, i64)> = None;
    for (start, end) in spans {
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

/// Whether the sorted, disjoint blocks cover `[start, end)` without a hole.
fn covers(blocks: &[TimeBlock], start: time::OffsetDateTime, end: time::OffsetDateTime) -> bool {
    let mut cursor = start;
    for block in blocks {
        if block.ended_at <= cursor {
            continue;
        }
        if block.started_at > cursor {
            return false;
        }
        cursor = block.ended_at;
        if cursor >= end {
            return true;
        }
    }
    cursor >= end
}

fn assert_disjoint_and_evidenced(blocks: &[TimeBlock], event_ids: &BTreeSet<String>) {
    for block in blocks {
        assert!(block.ended_at > block.started_at);
        assert!(!block.event_ids.is_empty());
        for id in &block.event_ids {
            assert!(event_ids.contains(id), "block cites unknown event {id}");
        }
    }
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
fn the_gold_set_segments_into_disjoint_blocks_that_cover_its_union() {
    let events = corpus();
    assert_eq!(events.len(), 220);
    let actors = by_actor(&events);
    assert_eq!(actors.len(), 6);

    let mut total_blocks = 0usize;
    let mut anchors = 0usize;
    let mut passive = 0usize;
    for (actor, actor_events) in &actors {
        let blocks = segment(actor_events, &SegmentationConfig::default())
            .unwrap_or_else(|error| panic!("actor {actor}: {error}"));
        assert!(!blocks.is_empty(), "actor {actor} produced no blocks");

        let ids: BTreeSet<String> = actor_events.iter().map(|event| event.id.clone()).collect();
        assert_disjoint_and_evidenced(&blocks, &ids);

        // Every event's time must land in the blocks, even when another group claimed it first.
        for event in actor_events {
            assert!(
                covers(&blocks, event.started_at, event.ended_at),
                "actor {actor}: event {} is not covered by any block",
                event.id
            );
        }

        assert_eq!(
            blocks.iter().map(|block| block.duration_seconds).sum::<u64>(),
            union_seconds(actor_events),
            "actor {actor}: block time does not equal the union of its events"
        );

        for block in &blocks {
            match block.class {
                TimeBlockClass::Anchor => anchors += 1,
                TimeBlockClass::Passive => passive += 1,
            }
        }
        total_blocks += blocks.len();
    }

    assert!(total_blocks > actors.len());
    assert!(anchors > 0, "the corpus has meetings and calls");
    assert!(passive > 0, "the corpus has mail and document work");
}

#[test]
fn gold_set_segmentation_does_not_depend_on_event_order() {
    let events = corpus();
    for (actor, actor_events) in by_actor(&events) {
        let config = SegmentationConfig::default();
        let expected = segment(&actor_events, &config).expect("valid");
        let mut reversed = actor_events.clone();
        reversed.reverse();
        assert_eq!(
            segment(&reversed, &config).expect("valid"),
            expected,
            "actor {actor} changed when its events were reversed"
        );
    }
}
