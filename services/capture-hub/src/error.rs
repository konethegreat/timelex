// Written by Kone Tshivhinda & Claude
//! Errors the store reports, before the HTTP layer turns them into problem details.

use std::fmt;

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// The caller's input breaks a rule; the message is fixed text, safe to show.
    Invalid(&'static str),
    /// The caller lacks `capture:write`.
    Forbidden,
    /// The database failed. The detail is for the operator's log, never for the response.
    Internal(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Forbidden => write!(f, "missing permission: capture:write"),
            Self::Internal(message) => write!(f, "internal error: {message}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.to_string())
    }
}
