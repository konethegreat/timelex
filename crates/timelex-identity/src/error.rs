// Written by Kone Tshivhinda & Claude
//! Errors for identity rules. Plain enums with hand-written `Display`: no extra dependency, and
//! every variant is something a test can match on.

use std::fmt;

/// A rule was broken. The caller decides how to report it (for example as an HTTP 400 or 403).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// Tenant slug is not 3..=40 characters of lowercase letters, digits and hyphens, starting
    /// and ending with a letter or digit.
    InvalidSlug,
    /// A display name or key name was empty, too long, or contained control characters.
    InvalidName { max_chars: usize },
    /// Email is not a normalised ASCII address (see [`crate::Email`]).
    InvalidEmail,
    /// A role or permission string is not one we know.
    UnknownRole(String),
    /// A permission string is not one we know.
    UnknownPermission(String),
    /// The `agent` role cannot be combined with any other role.
    AgentRoleIsExclusive,
    /// A status string is not one we know.
    UnknownStatus(String),
    /// The acting user lacks the permission needed for this action.
    PermissionDenied(&'static str),
    /// Key scopes were empty.
    NoScopes,
    /// A key asked for a scope its owner's roles do not grant.
    ScopeNotGranted(String),
    /// Keys can only be issued to active users.
    KeyOwnerNotActive,
    /// The requested expiry is not in the future.
    ExpiryInPast,
    /// The change would leave the firm without an active admin.
    LastAdmin,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlug => write!(
                f,
                "slug must be 3 to 40 characters: lowercase letters, digits, hyphens, starting and ending with a letter or digit"
            ),
            Self::InvalidName { max_chars } => {
                write!(f, "name must be 1 to {max_chars} characters with no control characters")
            }
            Self::InvalidEmail => write!(f, "email must be a lowercase ASCII address like name@firm.example"),
            Self::UnknownRole(r) => write!(f, "unknown role: {r}"),
            Self::UnknownPermission(p) => write!(f, "unknown permission: {p}"),
            Self::AgentRoleIsExclusive => write!(f, "the agent role cannot be combined with other roles"),
            Self::UnknownStatus(s) => write!(f, "unknown status: {s}"),
            Self::PermissionDenied(p) => write!(f, "missing permission: {p}"),
            Self::NoScopes => write!(f, "an API key needs at least one scope"),
            Self::ScopeNotGranted(s) => write!(f, "scope {s} is not granted by the key owner's roles"),
            Self::KeyOwnerNotActive => write!(f, "keys can only be issued to active users"),
            Self::ExpiryInPast => write!(f, "expiry must be in the future"),
            Self::LastAdmin => write!(f, "a firm must keep at least one active admin"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Why a presented API-key token is malformed. Never include the token text in a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// Wrong length.
    BadLength,
    /// Does not start with `tlx_`.
    BadPrefix,
    /// The tenant part is not 32 lowercase hex characters.
    BadTenant,
    /// The `.` separator is missing, or the secret is not canonical base64url of 32 bytes.
    BadSecret,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLength => write!(f, "token has the wrong length"),
            Self::BadPrefix => write!(f, "token has the wrong prefix"),
            Self::BadTenant => write!(f, "token has a malformed tenant part"),
            Self::BadSecret => write!(f, "token has a malformed secret part"),
        }
    }
}

impl std::error::Error for TokenError {}

/// Why a well-formed key cannot be used right now. The service reports all of these to the caller
/// as the same 401, so a prober learns nothing; the distinction is for the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailure {
    /// The firm is suspended.
    TenantSuspended,
    /// The key was revoked.
    Revoked,
    /// The key's owner is not an active user.
    UserNotActive,
    /// The key has expired.
    Expired,
}

impl fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantSuspended => write!(f, "tenant suspended"),
            Self::Revoked => write!(f, "key revoked"),
            Self::UserNotActive => write!(f, "key owner not active"),
            Self::Expired => write!(f, "key expired"),
        }
    }
}

impl std::error::Error for AuthFailure {}
