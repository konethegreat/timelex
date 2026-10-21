// Written by Kone Tshivhinda & Claude
//! Validated text values: tenant slug, email address, display names.
//!
//! Each rule here is mirrored by a CHECK constraint in `services/identity/migrations/0001_identity.sql`.
//! The Rust check gives a clear error; the database check stops a bad row even if this code has a bug.
//! A test in `timelex-db` feeds the same samples to both and asserts they agree.

use crate::error::IdentityError;

/// A firm's short public name, for example `example-attorneys`.
///
/// 3 to 40 characters; lowercase ASCII letters, digits and hyphens; must start and end with a
/// letter or digit. Same rule as the `tenants_slug_format` constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantSlug(String);

impl TenantSlug {
    /// Validate and wrap a slug. Input is used as given: no trimming, no case folding, so what
    /// was approved is exactly what is stored.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        let b = s.as_bytes();
        let ok_char = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
        let valid = (3..=40).contains(&b.len())
            && ok_char(b[0])
            && ok_char(b[b.len() - 1])
            && b.iter().all(|&c| ok_char(c) || c == b'-');
        if valid {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdentityError::InvalidSlug)
        }
    }

    /// The slug text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A normalised email address: ASCII only, lowercase, no surrounding spaces, at most 254 characters.
///
/// Shape (same as the `users_email_normalised` constraint): exactly one `@`, no whitespace, a
/// non-empty local part, and a domain containing a dot that has at least one character on each side.
/// Internationalised (non-ASCII) addresses are **not supported yet**: lowercasing them in Rust and in
/// Postgres can disagree, and a disagreement would let a duplicate account in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    /// Trim, lowercase and validate. `"  Partner@Firm.EXAMPLE "` becomes `partner@firm.example`.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        let e = s.trim().to_ascii_lowercase();
        if !e.is_ascii() || e.len() > 254 || e.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(IdentityError::InvalidEmail);
        }
        let mut parts = e.split('@');
        let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(IdentityError::InvalidEmail);
        };
        let dot_inside = domain
            .char_indices()
            .any(|(i, c)| c == '.' && i >= 1 && i < domain.len() - 1);
        if local.is_empty() || !dot_inside {
            return Err(IdentityError::InvalidEmail);
        }
        Ok(Self(e))
    }

    /// The normalised address.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validate a human-readable name: 1 to `max_chars` characters after trimming, no control characters.
/// Returns the trimmed text.
pub fn validate_name(s: &str, max_chars: usize) -> Result<String, IdentityError> {
    let t = s.trim();
    let n = t.chars().count();
    if n == 0 || n > max_chars || t.chars().any(char::is_control) {
        return Err(IdentityError::InvalidName { max_chars });
    }
    Ok(t.to_owned())
}

/// Longest allowed tenant or user display name, in characters.
pub const MAX_DISPLAY_NAME: usize = 200;
/// Longest allowed API-key label, in characters.
pub const MAX_KEY_NAME: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_accepts_valid() {
        for s in ["abc", "example-attorneys", "a1b", "x-y-z", &"a".repeat(40)] {
            assert!(TenantSlug::parse(s).is_ok(), "{s}");
        }
    }

    #[test]
    fn slug_rejects_invalid() {
        let too_long = "a".repeat(41);
        for s in [
            "",
            "ab",
            "-abc",
            "abc-",
            "Abc",
            "a_b",
            "a b",
            "a.b",
            "abc\n",
            "ünï",
            too_long.as_str(),
        ] {
            assert_eq!(TenantSlug::parse(s), Err(IdentityError::InvalidSlug), "{s:?}");
        }
    }

    #[test]
    fn email_is_trimmed_and_lowercased() {
        let e = Email::parse("  Partner@Firm.EXAMPLE ").unwrap();
        assert_eq!(e.as_str(), "partner@firm.example");
    }

    #[test]
    fn email_rejects_malformed() {
        let long = format!("{}@firm.example", "a".repeat(250));
        for s in [
            "",
            "plain",
            "@firm.example",
            "a@b",
            "a@.example",
            "a@example.",
            "a@@b.example",
            "a@b@c.example",
            "a b@firm.example",
            "a@fi rm.example",
            "pärtner@firm.example",
            long.as_str(),
        ] {
            assert_eq!(Email::parse(s), Err(IdentityError::InvalidEmail), "{s:?}");
        }
    }

    #[test]
    fn email_accepts_dots_and_plus_in_local_part() {
        assert!(Email::parse("first.last+tag@mail.firm.example").is_ok());
    }

    #[test]
    fn name_is_trimmed_and_bounded() {
        assert_eq!(validate_name("  Acme Attorneys ", 200).unwrap(), "Acme Attorneys");
        assert!(validate_name("", 200).is_err());
        assert!(validate_name("   ", 200).is_err());
        assert!(validate_name(&"x".repeat(201), 200).is_err());
        assert!(validate_name(&"x".repeat(200), 200).is_ok());
        assert!(
            validate_name("bad\u{0007}name", 200).is_err(),
            "control character"
        );
        assert!(
            validate_name("line\nbreak", 200).is_err(),
            "newline is a control character"
        );
    }
}
