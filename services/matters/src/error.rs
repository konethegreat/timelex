// Written by Kone Tshivhinda & Claude
//! Errors the store reports, before the HTTP layer turns them into problem details.

use std::fmt;

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// The caller's input breaks a rule; the message is safe to show.
    Invalid(&'static str),
    /// No such object in the caller's firm. Objects of other firms look exactly the same.
    NotFound,
    /// The change would break a uniqueness rule; the message is safe to show.
    Conflict(&'static str),
    /// A CSV import row failed; the message is safe to show.
    Row {
        /// One-based row number, counting the header as row 1.
        row: usize,
        /// What is wrong with the row.
        message: &'static str,
    },
    /// The database failed. The detail is for the operator's log, never for the response.
    Internal(String),
}

impl StoreError {
    /// A stored value that violates a rule in this crate: a bug or a hand-edited row, not the caller's
    /// fault. The detail is for the operator.
    pub(crate) fn corrupt(message: &'static str) -> Self {
        Self::Internal(format!("stored value is not valid: {message}"))
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "{message}"),
            Self::NotFound => write!(f, "not found"),
            Self::Conflict(message) => write!(f, "{message}"),
            Self::Row { row, message } => write!(f, "row {row}: {message}"),
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

impl From<&'static str> for StoreError {
    fn from(message: &'static str) -> Self {
        Self::Invalid(message)
    }
}
