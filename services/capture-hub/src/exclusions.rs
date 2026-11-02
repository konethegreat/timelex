// Written by Kone Tshivhinda & Claude
//! Privacy exclusions: what a firm says must never become an activity event.
//!
//! A firm can exclude a **domain**, a **client** or a **matter**. A domain is checked at capture
//! time, before an event is stored: a signal whose participant is at an excluded domain is dropped.
//! Client and matter exclusions are recorded and exposed now; they take effect when attribution
//! exists (P3), because a raw signal does not yet name a client or a matter (ADR-0023).
//!
//! A domain match ignores case and a trailing dot, and covers subdomains: excluding `example.com`
//! also excludes `mail.example.com`, so a privacy filter cannot be bypassed by a sender using a
//! different host under the same name.

use std::collections::BTreeSet;

use serde::Serialize;
use time::OffsetDateTime;
use timelex_types::ActivityEvent;

use crate::error::StoreError;

/// What a firm can exclude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionKind {
    /// A mail or calendar domain. Enforced at capture.
    Domain,
    /// A client. Recorded now; enforced when attribution exists (P3).
    Client,
    /// A matter. Recorded now; enforced when attribution exists (P3).
    Matter,
}

impl ExclusionKind {
    /// The contract's spelling, also the stored value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Client => "client",
            Self::Matter => "matter",
        }
    }

    /// Parse the contract's spelling. Unknown text is refused, so a typo cannot become a silent
    /// no-op exclusion.
    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "domain" => Ok(Self::Domain),
            "client" => Ok(Self::Client),
            "matter" => Ok(Self::Matter),
            _ => Err(StoreError::Invalid("kind must be domain, client or matter")),
        }
    }
}

/// One exclusion a firm configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclusion {
    pub kind: ExclusionKind,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// The canonical stored form of an exclusion value.
pub fn normalize_value(kind: ExclusionKind, value: &str) -> Result<String, StoreError> {
    match kind {
        ExclusionKind::Domain => normalize_domain(value),
        ExclusionKind::Client | ExclusionKind::Matter => normalize_identifier(value),
    }
}

/// Lower-case, trim, drop one leading `@` and one trailing dot, then check the name is an ASCII
/// domain of at most 253 characters whose labels are 1 to 63 letters, digits or inner hyphens.
fn normalize_domain(raw: &str) -> Result<String, StoreError> {
    let trimmed = raw.trim();
    let without_at = trimmed.strip_prefix('@').unwrap_or(trimmed);
    let without_dot = without_at.strip_suffix('.').unwrap_or(without_at);
    let domain = without_dot.to_ascii_lowercase();
    let valid = !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if !valid {
        return Err(StoreError::Invalid("the domain is not a valid ASCII domain name"));
    }
    Ok(domain)
}

/// A client or matter identifier: trimmed, 1 to 128 bytes of letters, digits, `.`, `-`, `_` or `:`.
fn normalize_identifier(raw: &str) -> Result<String, StoreError> {
    let value = raw.trim();
    let valid = (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':'));
    if !valid {
        return Err(StoreError::Invalid(
            "a client or matter value must be 1 to 128 letters, digits, '.', '-', '_' or ':'",
        ));
    }
    Ok(value.to_owned())
}

/// An optional note, trimmed. An empty note is no note; a note is at most 500 characters and has no
/// control characters, so it cannot smuggle line breaks into an operator's log.
pub fn normalize_reason(reason: Option<&str>) -> Result<Option<String>, StoreError> {
    let Some(raw) = reason else { return Ok(None) };
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 500 {
        return Err(StoreError::Invalid("the reason must be at most 500 characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(StoreError::Invalid(
            "the reason must not contain control characters",
        ));
    }
    Ok(Some(value.to_owned()))
}

/// Whether `domain` is the excluded name or a subdomain of it. Both sides are normalised first.
pub fn matches_domain(domain: &str, excluded: &str) -> bool {
    let (Ok(domain), Ok(excluded)) = (normalize_domain(domain), normalize_domain(excluded)) else {
        return false;
    };
    if domain == excluded {
        return true;
    }
    domain
        .strip_suffix(&excluded)
        .is_some_and(|prefix| prefix.len() > 1 && prefix.ends_with('.'))
}

/// The distinct, normalised domains an event's participants are at. An address supplies the domain
/// when the connector left `domain` empty. Malformed domains are skipped: they cannot be matched
/// honestly, and refusing the whole signal for one bad participant would stall a connector.
pub fn participant_domains(event: &ActivityEvent) -> BTreeSet<String> {
    let Some(subject) = &event.subject else {
        return BTreeSet::new();
    };
    subject
        .participants
        .iter()
        .filter_map(|participant| {
            let raw = if !participant.domain.trim().is_empty() {
                Some(participant.domain.as_str())
            } else {
                participant
                    .address
                    .as_deref()
                    .and_then(|address| address.rsplit_once('@').map(|(_, domain)| domain))
            };
            raw.and_then(|domain| normalize_domain(domain).ok())
        })
        .collect()
}

/// Whether any participant of the event is at an excluded domain.
pub fn is_excluded(event: &ActivityEvent, excluded_domains: &[String]) -> bool {
    if excluded_domains.is_empty() {
        return false;
    }
    participant_domains(event).iter().any(|domain| {
        excluded_domains
            .iter()
            .any(|excluded| matches_domain(domain, excluded))
    })
}

#[cfg(test)]
mod tests {
    use time::{Duration, OffsetDateTime};
    use timelex_types as types;

    use super::*;

    fn event_with(participants: Vec<types::Participant>) -> ActivityEvent {
        let started = OffsetDateTime::now_utc();
        let mut event = ActivityEvent {
            schema_version: types::ACTIVITY_EVENT_SCHEMA_VERSION.to_owned(),
            id: String::new(),
            tenant_id: "tn_0191a2b3c4d57e6f8a9b0c1d2e3f4a5b".into(),
            source: types::Source {
                system: types::SourceSystem::M365Mail,
                source_id: "synthetic-message-a".into(),
            },
            kind: types::ActivityKind::EmailReceived,
            actor: types::Actor {
                user_id: "us_0191a2b3c4d57e6f".into(),
            },
            started_at: started,
            ended_at: started + Duration::minutes(1),
            captured_at: started + Duration::minutes(2),
            subject: Some(types::Subject {
                participants,
                ..Default::default()
            }),
            content_ref: None,
            privacy: types::Privacy {
                content_captured: false,
                redaction_applied: false,
            },
        };
        event.id = event.expected_id();
        event
    }

    fn participant(role: types::ParticipantRole, domain: &str, address: Option<&str>) -> types::Participant {
        types::Participant {
            role,
            domain: domain.into(),
            address: address.map(Into::into),
        }
    }

    #[test]
    fn a_domain_is_normalised_before_it_is_stored_or_compared() {
        assert_eq!(normalize_domain("  @Example.COM. ").unwrap(), "example.com");
        assert_eq!(normalize_domain("test.").unwrap(), "test");
        assert_eq!(
            normalize_domain("mail.example.co.za").unwrap(),
            "mail.example.co.za"
        );
        assert_eq!(normalize_domain("a-b.example.test").unwrap(), "a-b.example.test");
    }

    #[test]
    fn a_malformed_domain_is_refused() {
        for bad in [
            "",
            " ",
            "@",
            "a b.test",
            "exa/mple.test",
            "exämple.test",
            "a..test",
            "-a.test",
            "a-.test",
            ".test",
            "a@b.test",
            "a_b.test",
        ] {
            assert!(
                matches!(normalize_domain(bad), Err(StoreError::Invalid(_))),
                "{bad:?}"
            );
        }
        assert!(normalize_domain(&"a".repeat(254)).is_err());
        assert!(normalize_domain(&format!("{}.test", "a".repeat(64))).is_err());
    }

    #[test]
    fn an_excluded_domain_covers_its_subdomains_but_not_a_lookalike() {
        assert!(matches_domain("example.com", "example.com"));
        assert!(matches_domain("mail.example.com", "example.com"));
        assert!(matches_domain("MAIL.Example.COM.", "example.com"));
        assert!(!matches_domain("notexample.com", "example.com"));
        assert!(!matches_domain("example.com.evil.test", "example.com"));
        assert!(!matches_domain("com", "example.com"));
    }

    #[test]
    fn client_and_matter_values_keep_their_case_and_allow_ids() {
        assert_eq!(
            normalize_value(ExclusionKind::Client, " CL-2026-0042 ").unwrap(),
            "CL-2026-0042"
        );
        assert_eq!(
            normalize_value(ExclusionKind::Matter, "matter:0191a2b3").unwrap(),
            "matter:0191a2b3"
        );
        for bad in ["", " ", "two words", "a/b", "a@b", &"x".repeat(129)] {
            assert!(
                matches!(
                    normalize_value(ExclusionKind::Client, bad),
                    Err(StoreError::Invalid(_))
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_reason_is_trimmed_and_must_be_plain_text() {
        assert_eq!(normalize_reason(None).unwrap(), None);
        assert_eq!(normalize_reason(Some("  ")).unwrap(), None);
        assert_eq!(
            normalize_reason(Some(" Client asked us not to capture ")).unwrap(),
            Some("Client asked us not to capture".into())
        );
        assert!(matches!(
            normalize_reason(Some("line\nbreak")),
            Err(StoreError::Invalid(_))
        ));
        assert!(matches!(
            normalize_reason(Some(&"x".repeat(501))),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn participant_domains_come_from_the_domain_or_the_address() {
        let event = event_with(vec![
            participant(types::ParticipantRole::From, "Example.COM.", None),
            participant(types::ParticipantRole::To, "", Some("fee.earner@corp.test")),
            participant(types::ParticipantRole::Cc, "not a domain", None),
        ]);
        let domains = participant_domains(&event);
        assert_eq!(
            domains.into_iter().collect::<Vec<_>>(),
            ["corp.test", "example.com"]
        );
    }

    #[test]
    fn an_event_is_excluded_when_any_participant_is_at_an_excluded_domain() {
        let event = event_with(vec![
            participant(types::ParticipantRole::From, "mail.example.com", None),
            participant(types::ParticipantRole::To, "corp.test", None),
        ]);
        assert!(is_excluded(&event, &["example.com".into()]));
        assert!(!is_excluded(&event, &["other.test".into()]));
        assert!(!is_excluded(&event, &[]));
    }

    #[test]
    fn an_event_without_a_subject_is_not_excluded() {
        let mut event = event_with(vec![]);
        event.subject = None;
        assert!(!is_excluded(&event, &["example.com".into()]));
    }
}
