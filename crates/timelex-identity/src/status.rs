// Written by Kone Tshivhinda & Claude
//! Lifecycle states. Nothing is deleted: firms are suspended and people are disabled.

use crate::error::IdentityError;

/// Whether a firm may use TimeLex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    /// Normal.
    Active,
    /// Blocked: no key of this firm authenticates.
    Suspended,
}

impl TenantStatus {
    /// The stored spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }

    /// Parse the stored spelling.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        match s {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            other => Err(IdentityError::UnknownStatus(other.to_owned())),
        }
    }
}

/// Where a user is in their life with the firm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    /// Created but not yet confirmed. Cannot hold keys.
    Invited,
    /// Normal.
    Active,
    /// Blocked: their keys stop working.
    Disabled,
}

impl UserStatus {
    /// The stored spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    /// Parse the stored spelling.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        match s {
            "invited" => Ok(Self::Invited),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            other => Err(IdentityError::UnknownStatus(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip() {
        for s in [TenantStatus::Active, TenantStatus::Suspended] {
            assert_eq!(TenantStatus::parse(s.as_str()), Ok(s));
        }
        for s in [UserStatus::Invited, UserStatus::Active, UserStatus::Disabled] {
            assert_eq!(UserStatus::parse(s.as_str()), Ok(s));
        }
    }

    #[test]
    fn unknown_statuses_fail_closed() {
        assert!(TenantStatus::parse("Active").is_err());
        assert!(TenantStatus::parse("").is_err());
        assert!(UserStatus::parse("deleted").is_err());
    }
}
