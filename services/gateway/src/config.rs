// Written by Kone Tshivhinda & Claude
//! Gateway configuration, read from the environment. Secrets never live in files in the repository.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `TIMELEX_GATEWAY_SIGNING_KEY` | Base64 PKCS#8 Ed25519 private key (print one with `timelex-gateway keygen`) |
//! | `TIMELEX_GATEWAY_KEY_ID` | Key id placed in tokens (default `k1`) |
//! | `TIMELEX_OPERATOR_KEY_SHA256` | Hex SHA-256 of the platform operator's key; comma-separated to rotate |
//! | `TIMELEX_IDENTITY_URL` | Where the identity service listens (default `http://127.0.0.1:8081`) |

use std::time::Duration;

use sha2::{Digest, Sha256};
use timelex_auth::Signer;

/// How long the gateway waits for a service before answering 504.
pub const DEFAULT_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(5);

/// Everything the gateway needs to run.
pub struct Config {
    /// Signs internal tokens. The only place the private key lives.
    pub signer: Signer,
    /// SHA-256 hashes of accepted operator keys.
    pub operator_key_hashes: Vec<[u8; 32]>,
    /// Base URL of the identity service, without a trailing slash.
    pub identity_url: String,
    /// Upstream timeout.
    pub upstream_timeout: Duration,
}

/// Hash a presented key the way operator keys are stored in configuration.
pub fn sha256(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

/// Parse `hex,hex,...` into hashes. Each must be exactly 64 hex characters.
pub fn parse_operator_hashes(list: &str) -> Result<Vec<[u8; 32]>, String> {
    let mut out = Vec::new();
    for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if item.len() != 64 || !item.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("TIMELEX_OPERATOR_KEY_SHA256 must hold 64-character hex SHA-256 values".into());
        }
        let mut h = [0u8; 32];
        for (i, byte) in h.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&item[2 * i..2 * i + 2], 16).map_err(|_| "bad hex".to_string())?;
        }
        out.push(h);
    }
    if out.is_empty() {
        return Err("TIMELEX_OPERATOR_KEY_SHA256 is empty: no operator could ever create a firm".into());
    }
    Ok(out)
}

impl Config {
    /// Read the configuration from the environment, failing with a message that names what is missing.
    pub fn from_env() -> Result<Self, String> {
        let var = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));
        let kid = std::env::var("TIMELEX_GATEWAY_KEY_ID").unwrap_or_else(|_| "k1".into());
        let signer =
            Signer::from_base64(&kid, &var("TIMELEX_GATEWAY_SIGNING_KEY")?).map_err(|e| e.to_string())?;
        let identity_url =
            std::env::var("TIMELEX_IDENTITY_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into());
        Ok(Self {
            signer,
            operator_key_hashes: parse_operator_hashes(&var("TIMELEX_OPERATOR_KEY_SHA256")?)?,
            identity_url: identity_url.trim_end_matches('/').to_owned(),
            upstream_timeout: DEFAULT_UPSTREAM_TIMEOUT,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_a_known_answer() {
        // SHA-256("abc"), from FIPS 180-2.
        let h = sha256("abc");
        let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn operator_hashes_parse_and_round_trip() {
        let a = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let parsed = parse_operator_hashes(&format!("{a}, {}", a.to_uppercase())).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], sha256("abc"));
        assert_eq!(parsed[0], parsed[1], "hex case does not matter");
    }

    #[test]
    fn bad_operator_hashes_are_refused() {
        for bad in [
            "",
            " , ",
            "abc",
            &"g".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
        ] {
            assert!(parse_operator_hashes(bad).is_err(), "{bad:?}");
        }
    }
}
