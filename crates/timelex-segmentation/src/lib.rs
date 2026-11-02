// Written by Kone Tshivhinda & Claude
//! Deterministic segmentation of activity events into time blocks.
//!
//! A time block is a half-open interval on a single user's timeline. Blocks never overlap, so the
//! same second is never credited twice, and the blocks are derived from the events alone: no model,
//! no clock, no I/O. Billing rounding happens later, in `timelex-units`, when a block becomes an
//! entry.

mod interval;

use std::collections::BTreeMap;

use time::OffsetDateTime;
use timelex_types::{ActivityEvent, ActivityKind};

use interval::Interval;

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// Gap between two events of the same group that is still one block. Five minutes by default.
pub const DEFAULT_GAP_TOLERANCE_SECONDS: i64 = 5 * 60;

/// How blocks are assembled. Only the gap tolerance is configurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentationConfig {
    /// A gap of at most this many seconds between two intervals of the same group joins them.
    /// Must not be negative.
    pub gap_tolerance_seconds: i64,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            gap_tolerance_seconds: DEFAULT_GAP_TOLERANCE_SECONDS,
        }
    }
}

/// Why a set of events could not be segmented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentationError {
    /// The configured gap tolerance was negative.
    NegativeGapTolerance(i64),
    /// An event ended before it started.
    EndBeforeStart { event_id: String },
    /// One call may only segment one firm's events.
    MixedTenants { first: String, found: String },
    /// One call may only segment one person's events.
    MixedActors { first: String, found: String },
}

impl std::fmt::Display for SegmentationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeGapTolerance(seconds) => {
                write!(f, "gap tolerance must not be negative, got {seconds}")
            }
            Self::EndBeforeStart { event_id } => {
                write!(f, "event {event_id} ends before it starts")
            }
            Self::MixedTenants { first, found } => {
                write!(
                    f,
                    "events from tenants {first} and {found} cannot share a timeline"
                )
            }
            Self::MixedActors { first, found } => {
                write!(f, "events by actors {first} and {found} cannot share a timeline")
            }
        }
    }
}

impl std::error::Error for SegmentationError {}

/// Whether a block is hard time or background work.
///
/// Ordered so anchors sort first: when two groups claim the same second, the anchor keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeBlockClass {
    /// A meeting, call or manual entry: the user asserted the block or another system did.
    Anchor,
    /// Mail, document work, research or desktop focus: inferred background time.
    Passive,
}

/// Why events were grouped together.
///
/// Ordered for deterministic tie-breaking: thread before document before counterparty before a
/// lone event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GroupKey {
    /// `subject.threadRef`, the strongest grouping signal.
    Thread(String),
    /// `subject.documentRef`.
    Document(String),
    /// The sorted, lower-cased, de-duplicated participant domains.
    Counterparty(Vec<String>),
    /// No shared reference: the event stands alone.
    Event(String),
}

/// One contiguous, non-overlapping interval on a user's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeBlock {
    /// Start of the half-open interval.
    pub started_at: OffsetDateTime,
    /// End of the half-open interval.
    pub ended_at: OffsetDateTime,
    /// Whole seconds between start and end.
    pub duration_seconds: u64,
    /// Anchor or passive.
    pub class: TimeBlockClass,
    /// The group the block was built from.
    pub group: GroupKey,
    /// Ids of the events whose intervals touch this block, sorted.
    pub event_ids: Vec<String>,
}

/// Group and clip events into non-overlapping time blocks.
///
/// Every call handles one tenant and one actor. Groups are built from the shared reference the
/// events carry, merged across gaps within the tolerance, and then clipped so that a second is
/// credited to at most one block. Anchors are placed first, so a call during document work splits
/// that work into the time before and after the call.
///
/// The output is sorted by start and does not depend on the order of `events`.
pub fn segment(
    events: &[ActivityEvent],
    config: &SegmentationConfig,
) -> Result<Vec<TimeBlock>, SegmentationError> {
    if config.gap_tolerance_seconds < 0 {
        return Err(SegmentationError::NegativeGapTolerance(
            config.gap_tolerance_seconds,
        ));
    }

    let mut timeline: Option<(&str, &str)> = None;
    let mut grouped: BTreeMap<(TimeBlockClass, GroupKey), Vec<(Interval, String)>> = BTreeMap::new();

    for event in events {
        match timeline {
            Some((tenant, actor)) => {
                if event.tenant_id != tenant {
                    return Err(SegmentationError::MixedTenants {
                        first: tenant.to_owned(),
                        found: event.tenant_id.clone(),
                    });
                }
                if event.actor.user_id != actor {
                    return Err(SegmentationError::MixedActors {
                        first: actor.to_owned(),
                        found: event.actor.user_id.clone(),
                    });
                }
            }
            None => timeline = Some((&event.tenant_id, &event.actor.user_id)),
        }

        let Some(signal) = interval_of(event)? else {
            continue;
        };
        grouped
            .entry((class_of(event.kind), group_key(event)))
            .or_default()
            .push((signal, event.id.clone()));
    }

    let mut groups: Vec<Group> = grouped
        .into_iter()
        .map(|((class, key), events)| {
            let spans = interval::merge(
                events.iter().map(|(span, _)| *span).collect(),
                config.gap_tolerance_seconds,
            );
            Group {
                class,
                key,
                first_start: spans.first().map_or(i64::MAX, |span| span.start),
                spans,
                events,
            }
        })
        .collect();
    groups.sort_by(|left, right| {
        (left.class, left.first_start, &left.key).cmp(&(right.class, right.first_start, &right.key))
    });

    let mut claimed: Vec<Interval> = Vec::new();
    let mut blocks = Vec::new();
    for group in groups {
        for span in &group.spans {
            for remnant in interval::subtract(*span, &claimed) {
                let event_ids = evidence(&group.events, &remnant);
                debug_assert!(
                    !event_ids.is_empty(),
                    "a remnant of a merged group always comes from an event"
                );
                if event_ids.is_empty() {
                    continue;
                }
                blocks.push(TimeBlock {
                    started_at: timestamp(remnant.start),
                    ended_at: timestamp(remnant.end),
                    duration_seconds: (remnant.end - remnant.start) as u64,
                    class: group.class,
                    group: group.key.clone(),
                    event_ids,
                });
            }
        }
        let mut next = claimed;
        next.extend_from_slice(&group.spans);
        claimed = interval::merge(next, 0);
    }

    blocks.sort_by(|left, right| {
        (
            left.started_at,
            left.ended_at,
            left.class,
            &left.group,
            &left.event_ids,
        )
            .cmp(&(
                right.started_at,
                right.ended_at,
                right.class,
                &right.group,
                &right.event_ids,
            ))
    });
    Ok(blocks)
}

struct Group {
    class: TimeBlockClass,
    key: GroupKey,
    first_start: i64,
    spans: Vec<Interval>,
    events: Vec<(Interval, String)>,
}

fn class_of(kind: ActivityKind) -> TimeBlockClass {
    match kind {
        ActivityKind::Meeting | ActivityKind::Call | ActivityKind::ManualEntry => TimeBlockClass::Anchor,
        ActivityKind::EmailSent
        | ActivityKind::EmailReceived
        | ActivityKind::DocumentEdit
        | ActivityKind::DocumentView
        | ActivityKind::WebResearch
        | ActivityKind::DesktopFocus => TimeBlockClass::Passive,
    }
}

fn group_key(event: &ActivityEvent) -> GroupKey {
    if let Some(subject) = &event.subject {
        if let Some(thread) = non_empty(subject.thread_ref.as_deref()) {
            return GroupKey::Thread(thread.to_owned());
        }
        if let Some(document) = non_empty(subject.document_ref.as_deref()) {
            return GroupKey::Document(document.to_owned());
        }
        let mut domains: Vec<String> = subject
            .participants
            .iter()
            .filter_map(|participant| {
                let domain = participant
                    .domain
                    .trim()
                    .trim_end_matches('.')
                    .to_ascii_lowercase();
                (!domain.is_empty()).then_some(domain)
            })
            .collect();
        if !domains.is_empty() {
            domains.sort_unstable();
            domains.dedup();
            return GroupKey::Counterparty(domains);
        }
    }
    GroupKey::Event(event.id.clone())
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// The whole-second interval an event claims: start rounded down, end rounded up.
///
/// A point event (end equal to start) claims nothing.
fn interval_of(event: &ActivityEvent) -> Result<Option<Interval>, SegmentationError> {
    if event.ended_at < event.started_at {
        return Err(SegmentationError::EndBeforeStart {
            event_id: event.id.clone(),
        });
    }
    if event.ended_at == event.started_at {
        return Ok(None);
    }
    let start_nanos = event.started_at.unix_timestamp_nanos();
    let end_nanos = event.ended_at.unix_timestamp_nanos();
    // `time` spans years -9999..=9999, so both second values fit comfortably in i64.
    let start = start_nanos.div_euclid(NANOS_PER_SECOND) as i64;
    let mut end = end_nanos.div_euclid(NANOS_PER_SECOND) as i64;
    if end_nanos.rem_euclid(NANOS_PER_SECOND) != 0 {
        end += 1;
    }
    Ok((end > start).then_some(Interval { start, end }))
}

fn timestamp(seconds: i64) -> OffsetDateTime {
    // The value came from an `OffsetDateTime`, so it is inside the representable range.
    OffsetDateTime::from_unix_timestamp(seconds).expect("seconds derived from an OffsetDateTime")
}

fn evidence(events: &[(Interval, String)], remnant: &Interval) -> Vec<String> {
    let mut ids: Vec<String> = events
        .iter()
        .filter(|(span, _)| span.overlaps(remnant))
        .map(|(_, id)| id.clone())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}
