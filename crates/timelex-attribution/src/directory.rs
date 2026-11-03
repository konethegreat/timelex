// Written by Kone Tshivhinda & Claude
//! The firm's clients, matters and explicit aliases, as attribution sees them.
//!
//! The directory is a validated snapshot, not a store: the matters service owns the records and a
//! caller passes them in. Validation happens once, at construction, so a resolution can never see
//! two matters claiming one reference or two clients claiming one domain (ADR-0027).

use std::collections::BTreeMap;

/// Whether a matter may still receive new work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatterStatus {
    Open,
    Closed,
}

/// The three explicit alias kinds the matters service stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AliasKind {
    MatterReference,
    ClientName,
    ClientDomain,
}

/// One client as attribution needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRecord {
    pub client_id: String,
    /// The client's mail domain, if the firm recorded one.
    pub domain: Option<String>,
}

impl ClientRecord {
    pub fn new(client_id: impl Into<String>, domain: Option<&str>) -> Self {
        Self {
            client_id: client_id.into(),
            domain: domain.map(str::to_owned),
        }
    }
}

/// One matter as attribution needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatterRecord {
    pub matter_id: String,
    pub client_id: String,
    /// The reference people write on documents; canonicalised on the way into the directory.
    pub reference: String,
    pub status: MatterStatus,
}

/// One explicit alias. A `matter_reference` names a matter; the client kinds name a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasRecord {
    pub kind: AliasKind,
    pub value: String,
    pub matter_id: Option<String>,
    pub client_id: Option<String>,
}

/// Why a directory cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryError {
    DuplicateClient(String),
    DuplicateMatter(String),
    UnknownClient(String),
    /// A matter's reference is empty after trimming.
    EmptyReference(String),
    DuplicateReference(String),
    DuplicateDomain(String),
    /// An alias has no target, or names a client or matter that is not in the directory.
    AliasTargetMissing,
    /// An alias value is empty after canonicalisation.
    EmptyAlias {
        kind: AliasKind,
    },
    /// A matter alias names a client or a client alias names a matter.
    AliasTargetMismatch,
    DuplicateAlias {
        kind: AliasKind,
        value: String,
    },
}

impl std::fmt::Display for DirectoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateClient(id) => write!(f, "client {id} appears twice"),
            Self::DuplicateMatter(id) => write!(f, "matter {id} appears twice"),
            Self::UnknownClient(id) => write!(f, "matter or alias names unknown client {id}"),
            Self::EmptyReference(id) => write!(f, "matter {id} has an empty reference"),
            Self::DuplicateReference(value) => write!(f, "more than one record claims reference {value:?}"),
            Self::DuplicateDomain(value) => write!(f, "more than one client claims domain {value:?}"),
            Self::AliasTargetMissing => write!(f, "an alias has no target, or names an unknown record"),
            Self::EmptyAlias { kind } => write!(f, "a {kind:?} alias has an empty value"),
            Self::AliasTargetMismatch => write!(f, "a matter alias names a client or the reverse"),
            Self::DuplicateAlias { kind, value } => {
                write!(f, "more than one {kind:?} alias claims value {value:?}")
            }
        }
    }
}

impl std::error::Error for DirectoryError {}

/// A validated snapshot of the firm's records.
#[derive(Debug, Clone)]
pub struct Directory {
    matters: BTreeMap<String, MatterEntry>,
    clients: BTreeMap<String, ClientEntry>,
    reference_to_matter: BTreeMap<String, String>,
    domain_to_client: BTreeMap<String, String>,
    name_to_client: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MatterEntry {
    pub(crate) client_id: String,
    pub(crate) status: MatterStatus,
}

#[derive(Debug, Clone)]
struct ClientEntry {
    matter_ids: Vec<String>,
}

impl Directory {
    /// Validate and canonicalise the firm's records.
    ///
    /// References are trimmed and lower-cased; domains are trimmed, lower-cased and lose one
    /// trailing dot. Values that collide are an error because a hard key with two targets would
    /// silently attribute work to whichever row was read first.
    pub fn new(
        clients: Vec<ClientRecord>,
        matters: Vec<MatterRecord>,
        aliases: Vec<AliasRecord>,
    ) -> Result<Self, DirectoryError> {
        let mut client_entries: BTreeMap<String, ClientEntry> = BTreeMap::new();
        let mut domain_to_client = BTreeMap::new();
        let mut name_to_client = BTreeMap::new();

        for client in clients {
            let client_id = client.client_id;
            if client_entries.contains_key(&client_id) {
                return Err(DirectoryError::DuplicateClient(client_id));
            }
            if let Some(domain) = client.domain.as_deref() {
                let domain = canonical_domain(domain);
                if !domain.is_empty()
                    && domain_to_client
                        .insert(domain.clone(), client_id.clone())
                        .is_some()
                {
                    return Err(DirectoryError::DuplicateDomain(domain));
                }
            }
            client_entries.insert(
                client_id,
                ClientEntry {
                    matter_ids: Vec::new(),
                },
            );
        }

        let mut matter_entries = BTreeMap::new();
        let mut reference_to_matter = BTreeMap::new();
        for matter in matters {
            let reference = canonical_reference(&matter.reference);
            if reference.is_empty() {
                return Err(DirectoryError::EmptyReference(matter.matter_id));
            }
            if matter_entries.contains_key(&matter.matter_id) {
                return Err(DirectoryError::DuplicateMatter(matter.matter_id));
            }
            let Some(client) = client_entries.get_mut(&matter.client_id) else {
                return Err(DirectoryError::UnknownClient(matter.client_id));
            };
            if reference_to_matter
                .insert(reference.clone(), matter.matter_id.clone())
                .is_some()
            {
                return Err(DirectoryError::DuplicateReference(reference));
            }
            client.matter_ids.push(matter.matter_id.clone());
            matter_entries.insert(
                matter.matter_id,
                MatterEntry {
                    client_id: matter.client_id,
                    status: matter.status,
                },
            );
        }

        for alias in aliases {
            let value = match alias.kind {
                AliasKind::MatterReference => canonical_reference(&alias.value),
                AliasKind::ClientName => canonical_reference(&alias.value),
                AliasKind::ClientDomain => canonical_domain(&alias.value),
            };
            if value.is_empty() {
                return Err(DirectoryError::EmptyAlias { kind: alias.kind });
            }
            match alias.kind {
                AliasKind::MatterReference => {
                    if alias.client_id.is_some() {
                        return Err(DirectoryError::AliasTargetMismatch);
                    }
                    let Some(matter_id) = alias.matter_id else {
                        return Err(DirectoryError::AliasTargetMissing);
                    };
                    if !matter_entries.contains_key(&matter_id) {
                        return Err(DirectoryError::AliasTargetMissing);
                    }
                    match reference_to_matter.get(&value) {
                        Some(existing) if existing != &matter_id => {
                            return Err(DirectoryError::DuplicateReference(value));
                        }
                        Some(_) => {}
                        None => {
                            reference_to_matter.insert(value, matter_id);
                        }
                    }
                }
                AliasKind::ClientName | AliasKind::ClientDomain => {
                    if alias.matter_id.is_some() {
                        return Err(DirectoryError::AliasTargetMismatch);
                    }
                    let Some(client_id) = alias.client_id else {
                        return Err(DirectoryError::AliasTargetMissing);
                    };
                    if !client_entries.contains_key(&client_id) {
                        return Err(DirectoryError::UnknownClient(client_id));
                    }
                    let map = match alias.kind {
                        AliasKind::ClientName => &mut name_to_client,
                        AliasKind::ClientDomain => &mut domain_to_client,
                        AliasKind::MatterReference => unreachable!("handled above"),
                    };
                    match map.get(&value) {
                        Some(existing) if existing != &client_id => {
                            return Err(DirectoryError::DuplicateAlias {
                                kind: alias.kind,
                                value,
                            });
                        }
                        Some(_) => {}
                        None => {
                            map.insert(value, client_id);
                        }
                    }
                }
            }
        }

        for entry in matter_entries.values() {
            if !client_entries.contains_key(&entry.client_id) {
                return Err(DirectoryError::UnknownClient(entry.client_id.clone()));
            }
        }
        for client in client_entries.values_mut() {
            client.matter_ids.sort();
        }

        Ok(Self {
            matters: matter_entries,
            clients: client_entries,
            reference_to_matter,
            domain_to_client,
            name_to_client,
        })
    }

    pub(crate) fn matter(&self, matter_id: &str) -> Option<&MatterEntry> {
        self.matters.get(matter_id)
    }

    /// Every canonical reference and the matter it names, in sorted order.
    pub(crate) fn references(&self) -> impl Iterator<Item = (&str, &str)> {
        self.reference_to_matter
            .iter()
            .map(|(reference, matter)| (reference.as_str(), matter.as_str()))
    }

    pub(crate) fn client_for_domain(&self, domain: &str) -> Option<&str> {
        self.domain_to_client.get(domain).map(String::as_str)
    }

    /// Every matter the client has, sorted.
    pub(crate) fn matters_of(&self, client_id: &str) -> &[String] {
        self.clients
            .get(client_id)
            .map(|client| client.matter_ids.as_slice())
            .unwrap_or(&[])
    }

    /// The client's open matters, sorted.
    pub(crate) fn open_matters_of(&self, client_id: &str) -> Vec<String> {
        self.matters_of(client_id)
            .iter()
            .filter(|matter_id| {
                self.matters
                    .get(*matter_id)
                    .is_some_and(|matter| matter.status == MatterStatus::Open)
            })
            .cloned()
            .collect()
    }

    /// The client named by a stored name alias, if any. Reserved for the retrieval stages.
    pub fn client_for_name(&self, name: &str) -> Option<&str> {
        self.name_to_client
            .get(&canonical_reference(name))
            .map(String::as_str)
    }
}

/// Trim and lower-case a reference. Values are ASCII in practice; Unicode is lower-cased too so a
/// non-ASCII name or value cannot bypass a collision check by changing case.
pub(crate) fn canonical_reference(value: &str) -> String {
    value.trim().to_lowercase()
}

/// Trim, lower-case and remove one trailing dot from a domain.
pub(crate) fn canonical_domain(value: &str) -> String {
    let trimmed = value.trim().to_lowercase();
    trimmed.strip_suffix('.').unwrap_or(&trimmed).to_owned()
}
