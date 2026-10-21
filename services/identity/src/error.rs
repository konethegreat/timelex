// Written by Kone Tshivhinda & Claude
//! Errors the store reports. The HTTP layer (later) maps them to the statuses in
//! `contracts/openapi/identity.v1.yaml`: `Invalid` is 400, `Forbidden` is 403, `NotFound` is 404,
//! `SlugTaken` is 409, every `AuthError` except `Internal` is the same 401, and `Internal` is 500.

use std::fmt;

use timelex_identity::{AuthFailure, IdentityError, TokenError};

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// A rule was broken (bad input, or the caller lacks a permission; see [`IdentityError`]).
    Rule(IdentityError),
    /// No such object in the caller's firm. Objects of other firms look exactly the same.
    NotFound,
    /// The firm slug is already used.
    SlugTaken,
    /// The database failed, or returned data this code does not understand.
    Internal(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(e) => write!(f, "{e}"),
            Self::NotFound => write!(f, "not found"),
            Self::SlugTaken => write!(f, "slug already taken"),
            Self::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<IdentityError> for StoreError {
    fn from(e: IdentityError) -> Self {
        Self::Rule(e)
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        // The sqlx message can contain query text, never a token; still keep it out of API responses.
        Self::Internal(e.to_string())
    }
}

impl StoreError {
    /// True when the caller was refused for lacking a permission (HTTP 403 rather than 400).
    pub fn is_forbidden(&self) -> bool {
        matches!(self, Self::Rule(IdentityError::PermissionDenied(_)))
    }
}

/// Why authentication failed. Everything except `Internal` is reported to the caller as the same 401.
#[derive(Debug)]
pub enum AuthError {
    /// The text is not a well-formed token.
    Malformed(TokenError),
    /// No key with this hash exists in the claimed firm.
    UnknownKey,
    /// The key exists but cannot be used (revoked, expired, owner or firm blocked).
    Rejected(AuthFailure),
    /// The database failed or returned data this code does not understand. Fails closed (no access).
    Internal(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "malformed token: {e}"),
            Self::UnknownKey => write!(f, "unknown key"),
            Self::Rejected(r) => write!(f, "key rejected: {r}"),
            Self::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<sqlx::Error> for AuthError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.to_string())
    }
}
