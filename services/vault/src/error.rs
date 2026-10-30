// Written by Kone Tshivhinda & Claude
//! Errors the store reports. The HTTP layer maps them to `contracts/openapi/vault.v1.yaml`:
//! `Invalid` is 400, `Forbidden` is 403, `NotFound` is 404, `Revoked` is 409, `Internal` is 500.
//!
//! No variant can carry a secret value: [`VaultError`] reasons are fixed strings and
//! [`StoreError::Internal`] is built from database messages and this crate's own text.

use std::fmt;

use timelex_vault::VaultError;

/// Why a vault store operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A rule was broken (bad value, label, host or kind).
    Invalid(VaultError),
    /// The caller lacks `settings:manage`.
    Forbidden,
    /// No such secret in the caller's firm. Secrets of other firms look exactly the same.
    NotFound,
    /// The secret exists but was revoked: test, rotate and open all refuse it.
    Revoked,
    /// The database failed, or returned data this code does not understand.
    Internal(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "{e}"),
            Self::Forbidden => write!(f, "missing permission: settings:manage"),
            Self::NotFound => write!(f, "not found"),
            Self::Revoked => write!(f, "the secret was revoked"),
            Self::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<VaultError> for StoreError {
    fn from(e: VaultError) -> Self {
        Self::Invalid(e)
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        // The sqlx message can contain query text, never a secret value; keep it out of API responses.
        Self::Internal(e.to_string())
    }
}
