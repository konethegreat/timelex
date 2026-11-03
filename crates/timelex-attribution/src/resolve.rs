// Written by Kone Tshivhinda & Claude
//! Stages 0 and 1: exact hard keys, then assignment history.
//!
//! Both stages are deterministic and cheap. A stage either names a matter, abstains, or asks for
//! review; it never guesses a candidate that the evidence does not support (ADR-0028).

use std::collections::{BTreeMap, BTreeSet};

use timelex_types::ActivityEvent;

use crate::directory::{canonical_domain, Directory, MatterStatus};

/// The evidence behind an assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// A matter reference in a subject, filename or thread.
    MatterReference,
    /// A participant domain that names exactly one client with one open matter.
    ClientDomain,
    /// A thread already assigned to a matter.
    HistoryThread,
    /// A document already assigned to a matter.
    HistoryDocument,
    /// A participant set whose client has exactly one open matter.
    HistoryClient,
}

/// Why a group needs a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewReason {
    /// The matter exists but is closed.
    ClosedMatter,
    /// The client is known, but several open matters fit.
    AmbiguousClient,
    /// Two hard keys name different matters.
    ConflictingHardKeys,
    /// The group's participants name more than one client.
    MultipleClients,
    /// Only some events in a group carry the reference, so the group may mix matters.
    MixedEvidence,
    /// Remembered assignments disagree.
    ConflictingHistory,
}

/// What stages 0 and 1 decided for one group of events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The matter may proceed to the deterministic checks and posting.
    Assigned {
        client_id: String,
        matter_id: String,
        evidence: Evidence,
    },
    /// A person must choose; the candidates are every matter the evidence can support.
    Review {
        reason: ReviewReason,
        client_id: Option<String>,
        candidate_matter_ids: Vec<String>,
    },
    /// No hard key or history applies. Later stages may still resolve the group.
    Unattributed,
}

/// Matter- and client-level assignments remembered from earlier groups.
///
/// Only thread and document history names a matter: one conversation or file belongs to one piece
/// of work. Participant history names a client, because the same contact appears across all of a
/// client's matters.
#[derive(Debug, Clone, Default)]
pub struct History {
    threads: BTreeMap<String, BTreeSet<String>>,
    documents: BTreeMap<String, BTreeSet<String>>,
    participants: BTreeMap<String, BTreeSet<String>>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.threads.is_empty() && self.documents.is_empty() && self.participants.is_empty()
    }

    /// Remember an assignment. Reviews and unattributed groups teach nothing, so they are ignored.
    pub fn record(&mut self, events: &[ActivityEvent], decision: &Decision) {
        let Decision::Assigned {
            client_id, matter_id, ..
        } = decision
        else {
            return;
        };
        for key in thread_keys(events) {
            self.threads.entry(key).or_default().insert(matter_id.clone());
        }
        for key in document_keys(events) {
            self.documents.entry(key).or_default().insert(matter_id.clone());
        }
        if let Some(key) = participants_key(events) {
            self.participants
                .entry(key)
                .or_default()
                .insert(client_id.clone());
        }
    }
}

/// Attribute one group of events: stage 0, then stage 1.
///
/// The caller passes the events segmentation put in one group (the evidence behind a time block).
/// For a thread or document group, one reference speaks for the group; for a counterparty group,
/// every event must carry the same reference, because such a group can hold work on several matters
/// for the same client.
pub fn attribute(events: &[ActivityEvent], directory: &Directory, history: &History) -> Decision {
    if events.is_empty() {
        return Decision::Unattributed;
    }
    let shape = Shape::of(events);
    hard_key(events, directory, shape)
        .or_else(|| client_domain(events, directory))
        .or_else(|| remembered(events, directory, history))
        .unwrap_or(Decision::Unattributed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Thread,
    Document,
    Other,
}

impl Shape {
    fn of(events: &[ActivityEvent]) -> Self {
        if shared_key(events, thread_key).is_some() {
            Self::Thread
        } else if shared_key(events, document_key).is_some() {
            Self::Document
        } else {
            Self::Other
        }
    }
}

fn hard_key(events: &[ActivityEvent], directory: &Directory, shape: Shape) -> Option<Decision> {
    let matched: Vec<BTreeSet<&str>> = events
        .iter()
        .map(|event| references_in(event, directory))
        .collect();
    let matters: BTreeSet<&str> = matched.iter().flatten().copied().collect();

    match matters.len() {
        0 => None,
        1 => {
            let matter_id = *matters.first().expect("one match");
            let entry = directory
                .matter(matter_id)
                .expect("a match names a directory matter");
            if shape == Shape::Other
                && matched
                    .iter()
                    .any(|event_matters| !event_matters.contains(matter_id))
            {
                return Some(review(
                    ReviewReason::MixedEvidence,
                    Some(&entry.client_id),
                    [matter_id.to_owned()]
                        .into_iter()
                        .chain(directory.open_matters_of(&entry.client_id)),
                ));
            }
            if entry.status == MatterStatus::Closed {
                return Some(review(
                    ReviewReason::ClosedMatter,
                    Some(&entry.client_id),
                    [matter_id.to_owned()],
                ));
            }
            let clients = participant_clients(events, directory);
            if clients.len() > 1 {
                return Some(review(
                    ReviewReason::MultipleClients,
                    None,
                    [matter_id.to_owned()].into_iter().chain(
                        clients
                            .iter()
                            .flat_map(|client| directory.matters_of(client).iter().cloned()),
                    ),
                ));
            }
            if let Some(client) = clients.first().copied() {
                if client != entry.client_id {
                    return Some(review(
                        ReviewReason::ConflictingHardKeys,
                        Some(&entry.client_id),
                        [matter_id.to_owned()]
                            .into_iter()
                            .chain(directory.matters_of(client).iter().cloned()),
                    ));
                }
            }
            Some(Decision::Assigned {
                client_id: entry.client_id.clone(),
                matter_id: matter_id.to_owned(),
                evidence: Evidence::MatterReference,
            })
        }
        _ => {
            let clients: BTreeSet<&str> = matters
                .iter()
                .filter_map(|matter_id| directory.matter(matter_id))
                .map(|matter| matter.client_id.as_str())
                .collect();
            Some(review(
                ReviewReason::ConflictingHardKeys,
                if clients.len() == 1 {
                    clients.first().copied()
                } else {
                    None
                },
                matters.into_iter().map(str::to_owned),
            ))
        }
    }
}

fn client_domain(events: &[ActivityEvent], directory: &Directory) -> Option<Decision> {
    let clients = participant_clients(events, directory);
    match clients.len() {
        0 => None,
        1 => {
            let client = *clients.first().expect("one client");
            Some(resolve_client(client, directory, Evidence::ClientDomain))
        }
        _ => Some(review(
            ReviewReason::MultipleClients,
            None,
            clients
                .iter()
                .flat_map(|client| directory.matters_of(client))
                .cloned(),
        )),
    }
}

fn remembered(events: &[ActivityEvent], directory: &Directory, history: &History) -> Option<Decision> {
    if let Some(decision) = remembered_matter(
        &history.threads,
        thread_keys(events),
        directory,
        Evidence::HistoryThread,
    ) {
        return Some(decision);
    }
    if let Some(decision) = remembered_matter(
        &history.documents,
        document_keys(events),
        directory,
        Evidence::HistoryDocument,
    ) {
        return Some(decision);
    }

    let clients: BTreeSet<&str> = participants_key(events)
        .and_then(|key| history.participants.get(&key))
        .map(|clients| clients.iter().map(String::as_str).collect())
        .unwrap_or_default();
    match clients.len() {
        0 => None,
        1 => {
            let client = *clients.first().expect("one client");
            Some(resolve_client(client, directory, Evidence::HistoryClient))
        }
        _ => Some(review(
            ReviewReason::ConflictingHistory,
            None,
            clients
                .iter()
                .flat_map(|client| directory.matters_of(client))
                .cloned(),
        )),
    }
}

fn remembered_matter(
    remembered: &BTreeMap<String, BTreeSet<String>>,
    keys: Vec<String>,
    directory: &Directory,
    evidence: Evidence,
) -> Option<Decision> {
    let matters: BTreeSet<&str> = keys
        .iter()
        .filter_map(|key| remembered.get(key))
        .flatten()
        .map(String::as_str)
        .collect();
    match matters.len() {
        0 => None,
        1 => {
            let matter_id = *matters.first().expect("one matter");
            let entry = directory
                .matter(matter_id)
                .expect("history names a directory matter");
            if entry.status == MatterStatus::Closed {
                return Some(review(
                    ReviewReason::ClosedMatter,
                    Some(&entry.client_id),
                    [matter_id.to_owned()],
                ));
            }
            Some(Decision::Assigned {
                client_id: entry.client_id.clone(),
                matter_id: matter_id.to_owned(),
                evidence,
            })
        }
        _ => Some(review(
            ReviewReason::ConflictingHistory,
            None,
            matters.into_iter().map(str::to_owned),
        )),
    }
}

/// Resolve a known client: one open matter is an assignment, several are a choice for a person.
fn resolve_client(client_id: &str, directory: &Directory, evidence: Evidence) -> Decision {
    let open = directory.open_matters_of(client_id);
    match open.len() {
        1 => Decision::Assigned {
            client_id: client_id.to_owned(),
            matter_id: open.into_iter().next().expect("one open matter"),
            evidence,
        },
        0 => review(
            ReviewReason::ClosedMatter,
            Some(client_id),
            directory.matters_of(client_id).iter().cloned(),
        ),
        _ => review(ReviewReason::AmbiguousClient, Some(client_id), open),
    }
}

fn review(
    reason: ReviewReason,
    client_id: Option<&str>,
    candidates: impl IntoIterator<Item = String>,
) -> Decision {
    let mut candidates: Vec<String> = candidates.into_iter().collect();
    candidates.sort();
    candidates.dedup();
    Decision::Review {
        reason,
        client_id: client_id.map(str::to_owned),
        candidate_matter_ids: candidates,
    }
}

fn participant_clients<'a>(events: &[ActivityEvent], directory: &'a Directory) -> BTreeSet<&'a str> {
    events
        .iter()
        .filter_map(|event| event.subject.as_ref())
        .flat_map(|subject| subject.participants.iter())
        .filter_map(|participant| directory.client_for_domain(&canonical_domain(&participant.domain)))
        .collect()
}

fn references_in<'a>(event: &ActivityEvent, directory: &'a Directory) -> BTreeSet<&'a str> {
    let Some(subject) = event.subject.as_ref() else {
        return BTreeSet::new();
    };
    let mut found = BTreeSet::new();
    for text in [
        subject.title.as_deref(),
        subject.thread_ref.as_deref(),
        subject.document_ref.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        for (reference, matter_id) in directory.references() {
            if contains_reference(text, reference) {
                found.insert(matter_id);
            }
        }
    }
    found
}

/// Whether `text` contains `reference` as a whole token.
///
/// `M-2026-0101` matches in `[M-2026-0101] lease` but not in `M-2026-01010`, so a longer reference
/// (or a word that merely starts with one) cannot silently claim a matter.
pub(crate) fn contains_reference(text: &str, reference: &str) -> bool {
    if reference.is_empty() {
        return false;
    }
    let text = text.to_lowercase();
    let mut from = 0;
    while let Some(position) = text[from..].find(reference) {
        let start = from + position;
        let end = start + reference.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric()) {
            return true;
        }
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn thread_key(event: &ActivityEvent) -> Option<&str> {
    event
        .subject
        .as_ref()?
        .thread_ref
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

fn document_key(event: &ActivityEvent) -> Option<&str> {
    event
        .subject
        .as_ref()?
        .document_ref
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

fn shared_key<'a>(
    events: &'a [ActivityEvent],
    key: impl Fn(&'a ActivityEvent) -> Option<&'a str>,
) -> Option<&'a str> {
    let mut keys = events.iter().map(key);
    let first = keys.next()??;
    if keys.all(|next| next == Some(first)) {
        Some(first)
    } else {
        None
    }
}

fn thread_keys(events: &[ActivityEvent]) -> Vec<String> {
    events.iter().filter_map(thread_key).map(str::to_owned).collect()
}

fn document_keys(events: &[ActivityEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(document_key)
        .map(str::to_owned)
        .collect()
}

fn participants_key(events: &[ActivityEvent]) -> Option<String> {
    let domains: BTreeSet<String> = events
        .iter()
        .filter_map(|event| event.subject.as_ref())
        .flat_map(|subject| subject.participants.iter())
        .map(|participant| canonical_domain(&participant.domain))
        .filter(|domain| !domain.is_empty())
        .collect();
    if domains.is_empty() {
        None
    } else {
        Some(domains.into_iter().collect::<Vec<_>>().join("\u{1f}"))
    }
}
