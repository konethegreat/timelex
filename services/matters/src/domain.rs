// Written by Kone Tshivhinda & Claude
//! Rules about clients, matters, aliases and rate cards (ADR-0027).
//!
//! Pure validation and canonicalisation: no database, no clock, no I/O. The store calls these
//! functions before it talks to Postgres, and the database repeats the important ones as constraints.

use serde::{Deserialize, Serialize};
use time::{Date, Month};

/// Longest identifier the firm may choose for a client or matter.
pub const MAX_ID: usize = 64;
/// Longest display name accepted.
pub const MAX_NAME: usize = 200;
/// Longest matter reference accepted.
pub const MAX_REFERENCE: usize = 64;
/// Longest alias value accepted (a domain or a display name).
pub const MAX_ALIAS_VALUE: usize = 320;
/// Upper bound on an hourly rate, in cents (R1 000 000.00 an hour).
pub const MAX_HOURLY_RATE_CENTS: i64 = 100_000_000;

fn trimmed(raw: &str) -> &str {
    raw.trim()
}

fn control_char(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn identifier(raw: &str, field: &'static str) -> Result<String, &'static str> {
    let value = trimmed(raw);
    if value.is_empty() || value.chars().count() > MAX_ID {
        return Err(match field {
            "clientId" => "clientId must be 1 to 64 characters",
            _ => "matterId must be 1 to 64 characters",
        });
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        return Err(match field {
            "clientId" => "clientId may contain only letters, digits, '.', '_', ':' and '-'",
            _ => "matterId may contain only letters, digits, '.', '_', ':' and '-'",
        });
    }
    Ok(value.to_owned())
}

/// A `clientId` as the firm's practice system writes it. Immutable once stored.
pub fn client_id(raw: &str) -> Result<String, &'static str> {
    identifier(raw, "clientId")
}

/// A `matterId` as the firm's practice system writes it. Immutable once stored.
pub fn matter_id(raw: &str) -> Result<String, &'static str> {
    identifier(raw, "matterId")
}

/// A human-readable name, trimmed and kept on one line.
pub fn display_name(raw: &str) -> Result<String, &'static str> {
    let value = trimmed(raw);
    if value.is_empty() || value.chars().count() > MAX_NAME {
        return Err("name must be 1 to 200 characters");
    }
    if control_char(value) {
        return Err("name must not contain control characters");
    }
    Ok(value.to_owned())
}

/// A matter reference, kept in the firm's spelling.
pub fn reference(raw: &str) -> Result<String, &'static str> {
    let value = trimmed(raw);
    if value.is_empty() || value.chars().count() > MAX_REFERENCE {
        return Err("reference must be 1 to 64 characters");
    }
    if control_char(value) {
        return Err("reference must not contain control characters");
    }
    Ok(value.to_owned())
}

/// The key two references are compared by: case-folded, so `M-1` and `m-1` are the same matter.
pub fn reference_key(raw: &str) -> String {
    trimmed(raw).to_lowercase()
}

/// A bare DNS domain, canonicalised: trimmed, lowercased and without the root dot.
pub fn domain(raw: &str) -> Result<String, &'static str> {
    let value = trimmed(raw).to_lowercase();
    let value = value.strip_suffix('.').unwrap_or(&value);
    if value.is_empty() || value.chars().count() > 253 {
        return Err("domain must be a bare DNS name with at least one dot");
    }
    let labels: Vec<&str> = value.split('.').collect();
    let valid = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.chars().count() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
    if !valid {
        return Err("domain must be a bare DNS name with at least one dot");
    }
    Ok(value.to_owned())
}

/// What an alias may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AliasKind {
    /// A matter reference as it appears in documents and emails.
    MatterReference,
    /// A client's display name.
    ClientName,
    /// A client's email domain.
    ClientDomain,
}

impl AliasKind {
    /// Parse the stored vocabulary.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        match trimmed(raw) {
            "matter_reference" => Ok(Self::MatterReference),
            "client_name" => Ok(Self::ClientName),
            "client_domain" => Ok(Self::ClientDomain),
            _ => Err("kind must be matter_reference, client_name or client_domain"),
        }
    }

    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MatterReference => "matter_reference",
            Self::ClientName => "client_name",
            Self::ClientDomain => "client_domain",
        }
    }
}

/// The value an alias is matched by, canonical for its kind. Everything an attribution lookup sees is
/// lowercased, so a signal's case never decides which matter it belongs to.
pub fn alias_value(kind: AliasKind, raw: &str) -> Result<String, &'static str> {
    let value = match kind {
        AliasKind::MatterReference => alias_text(reference(raw)?)?,
        AliasKind::ClientName => alias_text(display_name(raw)?)?,
        AliasKind::ClientDomain => domain(raw)?,
    };
    Ok(value.to_lowercase())
}

fn alias_text(value: String) -> Result<String, &'static str> {
    if value.chars().count() > MAX_ALIAS_VALUE {
        return Err("value must be 1 to 320 characters");
    }
    Ok(value)
}

/// A matter-reference alias must name a matter; a client alias must not. The other direction (a matter
/// alias that names another client's matter) is refused by the database's composite key.
pub fn alias_target(kind: AliasKind, names_matter: bool) -> Result<(), &'static str> {
    match (kind, names_matter) {
        (AliasKind::MatterReference, false) => Err("a matter_reference alias must name a matterId"),
        (AliasKind::MatterReference, true)
        | (AliasKind::ClientName, false)
        | (AliasKind::ClientDomain, false) => Ok(()),
        (AliasKind::ClientName | AliasKind::ClientDomain, true) => {
            Err("a client alias must not name a matterId")
        }
    }
}

/// A client's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientStatus {
    /// Active client.
    Active,
    /// Kept for history; no new matters should be opened.
    Inactive,
}

impl ClientStatus {
    /// Parse the stored vocabulary.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        match trimmed(raw) {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            _ => Err("status must be active or inactive"),
        }
    }

    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

/// A matter's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatterStatus {
    /// Open for new work.
    Open,
    /// Closed; signals that arrive later need review.
    Closed,
}

impl MatterStatus {
    /// Parse the stored vocabulary.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        match trimmed(raw) {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            _ => Err("status must be open or closed"),
        }
    }

    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

/// An ISO 8601 calendar date (`YYYY-MM-DD`), valid in the proleptic Gregorian calendar.
pub fn iso_date(raw: &str) -> Result<Date, &'static str> {
    let value = trimmed(raw);
    let bytes = value.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
    if !shaped {
        return Err("a date must be YYYY-MM-DD");
    }
    let number = |from: usize, to: usize| value[from..to].parse::<i32>().ok();
    let (Some(year), Some(month), Some(day)) = (number(0, 4), number(5, 7), number(8, 10)) else {
        return Err("a date must be YYYY-MM-DD");
    };
    let month = u8::try_from(month).ok().and_then(|m| Month::try_from(m).ok());
    month
        .and_then(|month| Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok())
        .ok_or("a date must be a real calendar date")
}

/// An hourly rate in whole cents (ZAR). Zero is allowed for pro bono work.
pub fn hourly_rate_cents(cents: i64) -> Result<i64, &'static str> {
    if !(0..=MAX_HOURLY_RATE_CENTS).contains(&cents) {
        return Err("hourlyRateCents must be 0 to 100000000");
    }
    Ok(cents)
}

/// Minutes in one billable unit.
pub fn unit_minutes(minutes: i64) -> Result<i64, &'static str> {
    if !(1..=60).contains(&minutes) {
        return Err("unitMinutes must be 1 to 60");
    }
    Ok(minutes)
}

/// Minimum units charged for a non-zero duration.
pub fn minimum_units(units: i64) -> Result<i64, &'static str> {
    if !(0..=60).contains(&units) {
        return Err("minimumUnits must be 0 to 60");
    }
    Ok(units)
}

/// How a partial unit is rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rounding {
    /// Always up.
    Up,
    /// Always down.
    Down,
    /// Nearest, with an exact half going up.
    Nearest,
}

impl Rounding {
    /// Parse the stored vocabulary.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        match trimmed(raw) {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "nearest" => Ok(Self::Nearest),
            _ => Err("rounding must be up, down or nearest"),
        }
    }

    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Nearest => "nearest",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    // ---------- identifiers ----------

    #[test]
    fn an_identifier_keeps_its_case_and_inner_separators() {
        assert_eq!(
            client_id("  CL-2026.001:Alpha_1  "),
            Ok("CL-2026.001:Alpha_1".into())
        );
        assert_eq!(matter_id("mt_f5909ad33820d983"), Ok("mt_f5909ad33820d983".into()));
    }

    #[test]
    fn an_identifier_is_bounded_and_composed_of_safe_characters() {
        assert_eq!(client_id(""), Err("clientId must be 1 to 64 characters"));
        assert_eq!(client_id("   "), Err("clientId must be 1 to 64 characters"));
        assert_eq!(
            matter_id(&"x".repeat(65)),
            Err("matterId must be 1 to 64 characters")
        );
        assert_eq!(
            matter_id("matter one"),
            Err("matterId may contain only letters, digits, '.', '_', ':' and '-'")
        );
        assert_eq!(
            client_id("client/1"),
            Err("clientId may contain only letters, digits, '.', '_', ':' and '-'")
        );
    }

    // ---------- names and references ----------

    #[test]
    fn a_display_name_is_trimmed_bounded_and_single_line() {
        assert_eq!(
            display_name("  Meridian Logistics  "),
            Ok("Meridian Logistics".into())
        );
        assert_eq!(display_name(""), Err("name must be 1 to 200 characters"));
        assert_eq!(
            display_name(&"n".repeat(201)),
            Err("name must be 1 to 200 characters")
        );
        assert_eq!(
            display_name("two\nlines"),
            Err("name must not contain control characters")
        );
    }

    #[test]
    fn a_reference_keeps_its_spelling_but_has_a_case_folded_key() {
        assert_eq!(reference("  M-2026-0101  "), Ok("M-2026-0101".into()));
        assert_eq!(reference_key("  M-2026-0101  "), "m-2026-0101");
        assert_eq!(reference(""), Err("reference must be 1 to 64 characters"));
        assert_eq!(
            reference("a\tb"),
            Err("reference must not contain control characters")
        );
    }

    // ---------- domains ----------

    #[test]
    fn a_domain_is_lowercased_and_stripped_of_a_trailing_dot() {
        assert_eq!(domain(" Meridian.Example. "), Ok("meridian.example".into()));
        assert_eq!(
            domain("mail.karoo-agri.example"),
            Ok("mail.karoo-agri.example".into())
        );
    }

    #[test]
    fn a_domain_is_a_bare_dns_name() {
        for bad in [
            "",
            "example",
            "https://example.com",
            "user@example.com",
            "example . com",
            "-bad.example",
            "bad-.example",
            ".example.com",
            &"a".repeat(64).to_string(),
        ] {
            assert!(domain(bad).is_err(), "{bad:?} was accepted");
        }
    }

    // ---------- alias kinds and canonical values ----------

    #[test]
    fn an_alias_kind_is_one_of_three() {
        assert_eq!(
            AliasKind::parse("matter_reference"),
            Ok(AliasKind::MatterReference)
        );
        assert_eq!(AliasKind::parse("client_name"), Ok(AliasKind::ClientName));
        assert_eq!(AliasKind::parse("client_domain"), Ok(AliasKind::ClientDomain));
        assert_eq!(
            AliasKind::parse("thread"),
            Err("kind must be matter_reference, client_name or client_domain")
        );
    }

    #[test]
    fn an_alias_value_is_canonical_for_its_kind() {
        assert_eq!(
            alias_value(AliasKind::ClientDomain, " Meridian.Example. "),
            Ok("meridian.example".into())
        );
        assert_eq!(
            alias_value(AliasKind::MatterReference, " M-2026-0101 "),
            Ok("m-2026-0101".into())
        );
        assert_eq!(
            alias_value(AliasKind::ClientName, " Meridian Logistics "),
            Ok("meridian logistics".into())
        );
    }

    #[test]
    fn a_matter_reference_alias_needs_a_matter_and_a_client_alias_must_not_name_one() {
        assert!(alias_target(AliasKind::MatterReference, false).is_err());
        assert!(alias_target(AliasKind::ClientDomain, true).is_err());
        assert_eq!(alias_target(AliasKind::MatterReference, true), Ok(()));
        assert_eq!(alias_target(AliasKind::ClientName, false), Ok(()));
    }

    // ---------- statuses ----------

    #[test]
    fn statuses_parse_exactly() {
        assert_eq!(ClientStatus::parse("active"), Ok(ClientStatus::Active));
        assert_eq!(ClientStatus::parse("inactive"), Ok(ClientStatus::Inactive));
        assert_eq!(
            ClientStatus::parse("Active"),
            Err("status must be active or inactive")
        );
        assert_eq!(MatterStatus::parse("open"), Ok(MatterStatus::Open));
        assert_eq!(MatterStatus::parse("closed"), Ok(MatterStatus::Closed));
        assert_eq!(MatterStatus::parse("live"), Err("status must be open or closed"));
    }

    // ---------- dates ----------

    #[test]
    fn a_date_is_iso_8601() {
        assert_eq!(
            iso_date("2026-11-02"),
            Ok(time::Date::from_calendar_date(2026, Month::November, 2).unwrap())
        );
        for bad in ["2026-13-01", "2026-02-30", "02/11/2026", "2026-1-2", ""] {
            assert!(iso_date(bad).is_err(), "{bad:?} was accepted");
        }
    }

    // ---------- rate cards ----------

    #[test]
    fn a_rate_is_whole_cents_within_a_sane_bound() {
        assert_eq!(hourly_rate_cents(280_000), Ok(280_000));
        assert_eq!(hourly_rate_cents(0), Ok(0));
        assert_eq!(
            hourly_rate_cents(-1),
            Err("hourlyRateCents must be 0 to 100000000")
        );
        assert_eq!(
            hourly_rate_cents(100_000_001),
            Err("hourlyRateCents must be 0 to 100000000")
        );
    }

    #[test]
    fn rate_card_units_are_bounded() {
        assert_eq!(unit_minutes(6), Ok(6));
        assert_eq!(unit_minutes(0), Err("unitMinutes must be 1 to 60"));
        assert_eq!(unit_minutes(61), Err("unitMinutes must be 1 to 60"));
        assert_eq!(minimum_units(0), Ok(0));
        assert_eq!(minimum_units(61), Err("minimumUnits must be 0 to 60"));
    }

    #[test]
    fn rounding_parses_to_the_stored_vocabulary() {
        assert_eq!(Rounding::parse("up"), Ok(Rounding::Up));
        assert_eq!(Rounding::parse("down"), Ok(Rounding::Down));
        assert_eq!(Rounding::parse("nearest"), Ok(Rounding::Nearest));
        assert_eq!(
            Rounding::parse("ceil"),
            Err("rounding must be up, down or nearest")
        );
    }
}
