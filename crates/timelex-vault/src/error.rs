// Written by Kone Tshivhinda & Claude
//! Errors for vault rules. Every variant is safe to show to the caller: none carries the secret
//! value, and a host is public information.

use std::fmt;

/// A vault rule was broken, or a sealed value would not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// The secret value itself was rejected. The reason never contains the value.
    InvalidValue(&'static str),
    /// The label was empty, too long, or contained control characters.
    InvalidLabel,
    /// An allowed-host list was rejected as a whole (empty, too long, duplicated).
    InvalidHosts(&'static str),
    /// One allowed host was rejected. `host` is safe to show: it is a host name, not a secret.
    InvalidHost { host: String, reason: &'static str },
    /// Sealing or opening failed: wrong key, tampered data, or data bound to another tenant,
    /// secret or kind.
    Crypto,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(reason) => write!(f, "{reason}"),
            Self::InvalidLabel => write!(f, "label must be 1 to 120 characters with no control characters"),
            Self::InvalidHosts(reason) => write!(f, "allowed hosts: {reason}"),
            Self::InvalidHost { host, reason } => write!(f, "allowed host {host:?} {reason}"),
            Self::Crypto => write!(f, "the secret could not be sealed or opened"),
        }
    }
}

impl std::error::Error for VaultError {}
