// Written by Kone Tshivhinda & Claude
//! Hash-chained audit log (ADR-0004 point 7, P1-14).
//!
//! A tenant's audit entries form one chain. Every entry stores the hash of the entry before it,
//! and its own hash covers that previous hash plus its fields, so editing, removing or reordering
//! an entry breaks every hash after it. [`verify`] recomputes a chain and returns its head; a
//! caller compares that head with an independently stored anchor (the vault keeps one in
//! `chain_heads`, for example) to catch a rewrite that recomputed the whole tail.
//!
//! Entries carry metadata only: what kind of thing happened, by whom, to which subject, and when.
//! `detail` comes from a fixed vocabulary each service chooses and never holds a secret value.
//!
//! [`entry_hash`] takes a service domain so two services cannot produce interchangeable hashes.
//! The byte layout is the vault's audit-chain v1 layout, so chains written before this library
//! existed still verify when the domain is `timelex-vault`.

use sha2::{Digest, Sha256};
use timelex_identity::UserId;
use uuid::Uuid;

/// A SHA-256 digest, and the stored shape of `prev_hash` and `entry_hash`.
pub type Hash = [u8; 32];

/// The previous hash of the first entry in a tenant's chain.
pub const GENESIS: Hash = [0; 32];

/// One entry to hash, borrowing its strings.
#[derive(Debug, Clone, Copy)]
pub struct AuditEntry<'a> {
    /// Position in the tenant's chain, starting at 1.
    pub seq: i64,
    /// When it happened, in microseconds since the Unix epoch (UTC).
    pub at_micros: i64,
    /// What happened, from the service's own vocabulary (for example `secret.created`).
    pub kind: &'a str,
    /// The person whose request caused the entry.
    pub actor: UserId,
    /// The thing the entry is about, as a UUID (a secret, a ledger entry, a matter).
    pub subject: Uuid,
    /// Free text from a fixed vocabulary; never a secret value.
    pub detail: &'a str,
}

/// One entry as read back from storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEntry {
    pub seq: i64,
    pub at_micros: i64,
    pub kind: String,
    pub actor: UserId,
    pub subject: Uuid,
    pub detail: String,
    pub prev_hash: Hash,
    pub entry_hash: Hash,
}

impl StoredEntry {
    /// This entry's hashed fields, borrowing from the stored row.
    pub fn entry(&self) -> AuditEntry<'_> {
        AuditEntry {
            seq: self.seq,
            at_micros: self.at_micros,
            kind: &self.kind,
            actor: self.actor,
            subject: self.subject,
            detail: &self.detail,
        }
    }
}

/// Why a chain does not verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainError {
    /// The next position in the chain is not the one found in storage.
    WrongSequence { expected: i64, found: i64 },
    /// The entry does not point at its predecessor's hash.
    WrongPreviousHash { seq: i64 },
    /// The entry's fields do not hash to the stored value.
    BadEntryHash { seq: i64 },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSequence { expected, found } => {
                write!(f, "expected audit entry {expected}, found {found}")
            }
            Self::WrongPreviousHash { seq } => {
                write!(f, "audit entry {seq} does not follow its predecessor")
            }
            Self::BadEntryHash { seq } => {
                write!(f, "audit entry {seq} does not match its stored hash")
            }
        }
    }
}

impl std::error::Error for ChainError {}

/// The hash stored with an entry: SHA-256 over the previous hash, the domain, and this entry.
///
/// Every field is length-prefixed so that no two different entries can hash the same bytes.
pub fn entry_hash(domain: &str, prev: &Hash, entry: &AuditEntry<'_>) -> Hash {
    let mut h = Sha256::new();
    h.update(prev);
    let mut push = |part: &[u8]| {
        h.update((part.len() as u32).to_le_bytes());
        h.update(part);
    };
    push(domain.as_bytes());
    push(b"audit-v1");
    push(entry.kind.as_bytes());
    push(&entry.seq.to_le_bytes());
    push(&entry.at_micros.to_le_bytes());
    push(entry.actor.as_uuid().as_bytes());
    push(entry.subject.as_bytes());
    push(entry.detail.as_bytes());
    h.finalize().into()
}

/// Verify a full chain from [`GENESIS`] and return its head hash.
///
/// An empty slice is a valid empty chain and returns [`GENESIS`].
pub fn verify(domain: &str, entries: &[StoredEntry]) -> Result<Hash, ChainError> {
    let mut prev = GENESIS;
    for (expected, stored) in (1..).zip(entries) {
        if stored.seq != expected {
            return Err(ChainError::WrongSequence {
                expected,
                found: stored.seq,
            });
        }
        if stored.prev_hash != prev {
            return Err(ChainError::WrongPreviousHash { seq: stored.seq });
        }
        let computed = entry_hash(domain, &stored.prev_hash, &stored.entry());
        if computed != stored.entry_hash {
            return Err(ChainError::BadEntryHash { seq: stored.seq });
        }
        prev = stored.entry_hash;
    }
    Ok(prev)
}
