// Written by Kone Tshivhinda & Claude
// Contract tests for the hash-chained audit log (P1-14).

use timelex_audit::{entry_hash, verify, AuditEntry, ChainError, Hash, StoredEntry, GENESIS};
use timelex_identity::UserId;
use uuid::Uuid;

fn actor(n: u128) -> UserId {
    UserId::from_uuid(Uuid::from_u128(n))
}

fn entry<'a>(seq: i64, kind: &'a str, detail: &'a str) -> AuditEntry<'a> {
    AuditEntry {
        seq,
        at_micros: 1_790_000_000_000_000 + seq,
        kind,
        actor: actor(33),
        subject: Uuid::from_u128(44),
        detail,
    }
}

/// A chain of `n` entries, correctly hashed.
fn chain(domain: &str, n: i64) -> Vec<StoredEntry> {
    let mut prev = GENESIS;
    (1..=n)
        .map(|seq| {
            let entry = entry(seq, if seq % 2 == 0 { "used" } else { "created" }, "detail");
            let hash = entry_hash(domain, &prev, &entry);
            let stored = StoredEntry {
                seq: entry.seq,
                at_micros: entry.at_micros,
                kind: entry.kind.to_string(),
                actor: entry.actor,
                subject: entry.subject,
                detail: entry.detail.to_string(),
                prev_hash: prev,
                entry_hash: hash,
            };
            prev = hash;
            stored
        })
        .collect()
}

fn hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn the_hash_matches_the_vault_chain_it_replaces() {
    // The vault's secret audit chain already has rows in development databases. Moving it onto
    // this library must not change any hash, or every existing chain stops verifying. This value
    // is the vault v0 implementation's output for the same entry.
    let entry = AuditEntry {
        seq: 1,
        at_micros: 1_790_000_000_000_000,
        kind: "created",
        actor: actor(33),
        subject: Uuid::from_u128(44),
        detail: "anthropic_api_key",
    };
    assert_eq!(
        hex(&entry_hash("timelex-vault", &GENESIS, &entry)),
        "769a1ab1102ccff39bd33f8ca087cedfeb023ebb481636ae246de9969926c1ef"
    );
}

#[test]
fn the_hash_is_deterministic_and_covers_every_field() {
    let base = entry_hash(
        "timelex-vault",
        &GENESIS,
        &entry(1, "created", "anthropic_api_key"),
    );
    assert_eq!(
        entry_hash(
            "timelex-vault",
            &GENESIS,
            &entry(1, "created", "anthropic_api_key")
        ),
        base
    );

    let variants = [
        entry(2, "created", "anthropic_api_key"),
        entry(1, "rotated", "anthropic_api_key"),
        entry(1, "created", "generic_api_key"),
        entry(1, "created", "anthropic_api_key "),
        AuditEntry {
            at_micros: 1_790_000_000_000_002,
            ..entry(1, "created", "anthropic_api_key")
        },
        AuditEntry {
            actor: actor(34),
            ..entry(1, "created", "anthropic_api_key")
        },
        AuditEntry {
            subject: Uuid::from_u128(45),
            ..entry(1, "created", "anthropic_api_key")
        },
    ];
    for variant in variants {
        assert_ne!(entry_hash("timelex-vault", &GENESIS, &variant), base);
    }
    assert_ne!(
        entry_hash("timelex-vault", &[1u8; 32], &entry(1, "created", "x")),
        base
    );
}

#[test]
fn different_services_never_share_a_hash_space() {
    let same = entry(1, "created", "detail");
    assert_ne!(
        entry_hash("timelex-vault", &GENESIS, &same),
        entry_hash("timelex-ledger", &GENESIS, &same)
    );
}

#[test]
fn fields_cannot_shift_bytes_into_each_other() {
    let a = AuditEntry {
        kind: "ab",
        detail: "c",
        ..entry(1, "ignored", "ignored")
    };
    let b = AuditEntry {
        kind: "a",
        detail: "bc",
        ..entry(1, "ignored", "ignored")
    };
    assert_ne!(
        entry_hash("timelex-vault", &GENESIS, &a),
        entry_hash("timelex-vault", &GENESIS, &b)
    );
}

#[test]
fn a_well_formed_chain_verifies_to_its_head() {
    let entries = chain("timelex-vault", 4);
    let head = entries[3].entry_hash;
    assert_eq!(verify("timelex-vault", &entries), Ok(head));
}

#[test]
fn an_empty_chain_is_valid_and_its_head_is_genesis() {
    assert_eq!(verify("timelex-vault", &[]), Ok(GENESIS));
}

#[test]
fn an_edited_entry_is_detected() {
    let mut entries = chain("timelex-vault", 4);
    entries[1].detail = "something else".to_string();

    assert_eq!(
        verify("timelex-vault", &entries),
        Err(ChainError::BadEntryHash { seq: 2 })
    );
}

#[test]
fn a_missing_entry_is_detected() {
    let mut entries = chain("timelex-vault", 4);
    entries.remove(1);

    assert_eq!(
        verify("timelex-vault", &entries),
        Err(ChainError::WrongSequence {
            expected: 2,
            found: 3
        })
    );
}

#[test]
fn a_chain_that_does_not_start_at_one_is_detected() {
    let entries = chain("timelex-vault", 2);
    assert_eq!(
        verify("timelex-vault", &entries[1..]),
        Err(ChainError::WrongSequence {
            expected: 1,
            found: 2
        })
    );
}

#[test]
fn reordered_entries_are_detected() {
    let mut entries = chain("timelex-vault", 3);
    entries.swap(1, 2);

    assert_eq!(
        verify("timelex-vault", &entries),
        Err(ChainError::WrongSequence {
            expected: 2,
            found: 3
        })
    );
}

#[test]
fn a_forged_previous_hash_is_detected() {
    let mut entries = chain("timelex-vault", 3);
    entries[1].prev_hash = [9u8; 32];

    assert_eq!(
        verify("timelex-vault", &entries),
        Err(ChainError::WrongPreviousHash { seq: 2 })
    );
}

#[test]
fn a_rehashed_tail_is_caught_by_comparing_the_head_to_an_anchored_hash() {
    let entries = chain("timelex-vault", 4);
    let anchored_head = entries[3].entry_hash;

    // An attacker with write access rewrites entry 2 and recomputes every hash after it. The
    // chain is internally consistent, but its head no longer matches the separately stored one.
    let mut rewritten = entries;
    rewritten[1].detail = "something else".to_string();
    let mut prev = rewritten[0].entry_hash;
    for stored in &mut rewritten[1..] {
        stored.prev_hash = prev;
        let entry = stored.entry();
        stored.entry_hash = entry_hash("timelex-vault", &prev, &entry);
        prev = stored.entry_hash;
    }

    let live_head = verify("timelex-vault", &rewritten).expect("internally consistent");
    assert_ne!(live_head, anchored_head);
}
