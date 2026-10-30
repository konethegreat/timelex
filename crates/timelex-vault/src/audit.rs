// Written by Kone Tshivhinda & Claude
//! The tamper-evident access log (ADR-0004 point 7).
//!
//! Each tenant has one audit chain: every entry stores the hash of the entry before it, and its own
//! hash covers that previous hash plus its fields. Changing or removing an entry breaks every hash
//! after it, so a rewritten history can be detected by recomputing from [`GENESIS`].
//!
//! The chain holds metadata only: which action, by which user, on which secret, and when. A detail
//! string may say which kind was stored or whether a provider test passed; it never holds a value.

use sha2::{Digest, Sha256};
use timelex_identity::UserId;

use crate::secret::SecretId;

/// The previous hash of the first entry in a tenant's chain.
pub const GENESIS: [u8; 32] = [0; 32];

/// What happened to a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// A secret was created (or replaced, which destroys the old value).
    Created,
    /// A secret's value was rotated; the old data key is destroyed.
    Rotated,
    /// A secret was revoked; its value is destroyed and the row is kept as metadata.
    Revoked,
    /// A provider check was run. The detail says `ok` or how it failed.
    Tested,
    /// A caller was given the plaintext (a lease). Not used in v0: leases arrive with `mcp-host`.
    Used,
}

impl AuditAction {
    /// Every action, for tests and listings.
    pub const ALL: [AuditAction; 5] = [
        AuditAction::Created,
        AuditAction::Rotated,
        AuditAction::Revoked,
        AuditAction::Tested,
        AuditAction::Used,
    ];

    /// The stored name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Rotated => "rotated",
            Self::Revoked => "revoked",
            Self::Tested => "tested",
            Self::Used => "used",
        }
    }

    /// Parse a stored name.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

/// One entry to hash and append.
#[derive(Debug, Clone, Copy)]
pub struct AuditRecord<'a> {
    /// Position in the tenant's chain, starting at 1.
    pub seq: i64,
    /// When it happened, in microseconds since the Unix epoch (UTC).
    pub at_micros: i64,
    pub action: AuditAction,
    /// The person whose request caused the action.
    pub actor: UserId,
    pub secret: SecretId,
    /// Free text from a fixed vocabulary the service chooses; never a secret value.
    pub detail: &'a str,
}

/// The hash stored with this entry: SHA-256 over the previous hash and this entry's fields.
///
/// Every field is length-prefixed so that no two different entries can hash the same bytes (there
/// is no separator for a detail string to forge its way past).
pub fn entry_hash(prev: &[u8; 32], record: &AuditRecord<'_>) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev);
    let mut push = |part: &[u8]| {
        h.update((part.len() as u32).to_le_bytes());
        h.update(part);
    };
    push(b"timelex-vault");
    push(b"audit-v1");
    push(record.action.as_str().as_bytes());
    push(&record.seq.to_le_bytes());
    push(&record.at_micros.to_le_bytes());
    push(record.actor.as_uuid().as_bytes());
    push(record.secret.as_uuid().as_bytes());
    push(record.detail.as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn record() -> AuditRecord<'static> {
        AuditRecord {
            seq: 1,
            at_micros: 1_790_000_000_000_000,
            action: AuditAction::Created,
            actor: UserId::from_uuid(Uuid::from_u128(33)),
            secret: SecretId::from_uuid(Uuid::from_u128(44)),
            detail: "anthropic_api_key",
        }
    }

    #[test]
    fn every_action_round_trips_and_names_are_distinct() {
        for (i, a) in AuditAction::ALL.iter().enumerate() {
            assert_eq!(AuditAction::parse(a.as_str()), Some(*a));
            for b in &AuditAction::ALL[i + 1..] {
                assert_ne!(a.as_str(), b.as_str());
            }
        }
        assert_eq!(AuditAction::parse("ACreated"), None);
    }

    #[test]
    fn the_hash_is_deterministic_and_covers_every_field() {
        let base = entry_hash(&GENESIS, &record());
        assert_eq!(entry_hash(&GENESIS, &record()), base);
        let variants = [
            AuditRecord { seq: 2, ..record() },
            AuditRecord {
                at_micros: 1_790_000_000_000_001,
                ..record()
            },
            AuditRecord {
                action: AuditAction::Rotated,
                ..record()
            },
            AuditRecord {
                actor: UserId::from_uuid(Uuid::from_u128(34)),
                ..record()
            },
            AuditRecord {
                secret: SecretId::from_uuid(Uuid::from_u128(45)),
                ..record()
            },
            AuditRecord {
                detail: "generic_api_key",
                ..record()
            },
            AuditRecord {
                detail: "anthropic_api_key ",
                ..record()
            },
        ];
        for v in variants {
            assert_ne!(entry_hash(&GENESIS, &v), base, "{v:?}");
        }
        assert_ne!(entry_hash(&[1u8; 32], &record()), base);
    }

    #[test]
    fn a_chain_detects_a_changed_middle_entry() {
        let entries: Vec<AuditRecord> = (1..=4)
            .map(|seq| AuditRecord {
                seq,
                at_micros: 1_790_000_000_000_000 + seq,
                action: AuditAction::ALL[seq as usize - 1],
                detail: "",
                ..record()
            })
            .collect();
        let hashes = |list: &[AuditRecord<'_>]| {
            let mut prev = GENESIS;
            list.iter()
                .map(|r| {
                    prev = entry_hash(&prev, r);
                    prev
                })
                .collect::<Vec<_>>()
        };
        let good = hashes(&entries);
        let mut tampered = entries.clone();
        tampered[1].action = AuditAction::Used;
        let bad = hashes(&tampered);
        assert_eq!(good[0], bad[0], "entries before the change still match");
        assert_ne!(good[2], bad[2], "every later hash changes");
        assert_ne!(good[3], bad[3]);
    }

    #[test]
    fn detail_cannot_shift_bytes_between_fields() {
        // Two entries whose concatenated fields look alike must still hash differently.
        let a = AuditRecord {
            detail: "x",
            ..record()
        };
        let b = AuditRecord {
            detail: "",
            ..record()
        };
        assert_ne!(entry_hash(&GENESIS, &a), entry_hash(&GENESIS, &b));
    }
}
