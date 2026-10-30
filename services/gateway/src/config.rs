// Written by Kone Tshivhinda & Claude
//! Gateway configuration, read from the environment. Secrets never live in files in the repository.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `TIMELEX_GATEWAY_SIGNING_KEY` | Base64 PKCS#8 Ed25519 private key (print one with `timelex-gateway keygen`) |
//! | `TIMELEX_GATEWAY_KEY_ID` | Key id placed in tokens (default `k1`) |
//! | `TIMELEX_OPERATOR_KEY_SHA256` | Hex SHA-256 of the platform operator's key; comma-separated to rotate |
//! | `TIMELEX_SERVICE_URLS` | `name=url` pairs (default `identity=http://127.0.0.1:8081`); `identity` is required |
//! | `TIMELEX_RATE_LIMIT_PER_MINUTE` | Firm request budget (default 600) |
//! | `TIMELEX_RATE_LIMIT_BURST` | Firm burst allowance (default 120) |
//! | `TIMELEX_GATEWAY_INTERNAL_ADDR` | Loopback listener for `/internal/usage` (default `127.0.0.1:8089`) |

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use sha2::{Digest, Sha256};
use timelex_auth::Signer;

/// How long the gateway waits for a service before answering 504.
pub const DEFAULT_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(5);
/// Firm requests allowed a minute unless configured otherwise.
pub const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 600;
/// Firm burst allowance unless configured otherwise.
pub const DEFAULT_RATE_LIMIT_BURST: u32 = 120;
/// The platform operator's budget: few calls, and no reason to make many.
pub const OPERATOR_RATE_LIMIT_PER_MINUTE: u32 = 60;
/// The operator's burst allowance.
pub const OPERATOR_RATE_LIMIT_BURST: u32 = 10;
/// Where the internal usage listener binds unless configured otherwise.
pub const DEFAULT_INTERNAL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8089);
/// An upper bound so a slipped decimal point cannot mean "effectively unlimited".
pub const MAX_RATE_LIMIT: u32 = 1_000_000;

/// Everything the gateway needs to run.
pub struct Config {
    /// Signs internal tokens. The only place the private key lives.
    pub signer: Signer,
    /// SHA-256 hashes of accepted operator keys.
    pub operator_key_hashes: Vec<[u8; 32]>,
    /// Base URLs of the services, by name, without a trailing slash. `identity` is always present.
    pub service_urls: HashMap<String, String>,
    /// Upstream timeout.
    pub upstream_timeout: Duration,
    /// Firm requests allowed a minute.
    pub rate_limit_per_minute: u32,
    /// Firm burst allowance.
    pub rate_limit_burst: u32,
    /// Where `/internal/usage` is served. Meant for loopback only.
    pub internal_addr: SocketAddr,
}

impl Config {
    /// The base URL of a configured service, without a trailing slash.
    pub fn service_url(&self, name: &str) -> Option<&str> {
        self.service_urls.get(name).map(String::as_str)
    }
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

/// Parse `name=url,len=url` into base URLs, stripping a trailing slash. `identity` is required: without
/// it the gateway could not authenticate anyone.
pub fn parse_service_urls(list: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (name, url) = entry.split_once('=').ok_or(
            "TIMELEX_SERVICE_URLS must be comma-separated name=url pairs, for example identity=http://127.0.0.1:8081",
        )?;
        let name = name.trim();
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
            return Err(format!(
                "TIMELEX_SERVICE_URLS has an invalid service name {name:?}: use lowercase letters and '-'"
            ));
        }
        let url = url.trim().trim_end_matches('/');
        let parsed = reqwest::Url::parse(url)
            .map_err(|_| format!("TIMELEX_SERVICE_URLS: {name}'s URL is not a valid URL"))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(format!(
                "TIMELEX_SERVICE_URLS: {name}'s URL must be an absolute http or https URL"
            ));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(format!(
                "TIMELEX_SERVICE_URLS: {name}'s URL must not carry a query string or fragment"
            ));
        }
        if out.insert(name.to_owned(), url.to_owned()).is_some() {
            return Err(format!("TIMELEX_SERVICE_URLS names {name} more than once"));
        }
    }
    if !out.contains_key("identity") {
        return Err(
            "TIMELEX_SERVICE_URLS must include identity=<url>: the gateway authenticates every tenant call through it"
                .into(),
        );
    }
    Ok(out)
}

/// Parse a positive whole-number budget. Zero is refused: it would lock a firm out with no way back.
pub fn parse_budget(name: &str, value: &str) -> Result<u32, String> {
    let n: u32 = value
        .trim()
        .parse()
        .map_err(|_| format!("{name} must be a whole number"))?;
    if n == 0 || n > MAX_RATE_LIMIT {
        return Err(format!("{name} must be between 1 and {MAX_RATE_LIMIT}"));
    }
    Ok(n)
}

/// Parse a `host:port` listener address.
pub fn parse_addr(name: &str, value: &str) -> Result<SocketAddr, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("{name} must be host:port, for example 127.0.0.1:8089"))
}

impl Config {
    /// Read the configuration from the environment, failing with a message that names what is missing.
    pub fn from_env() -> Result<Self, String> {
        let var = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));
        let kid = std::env::var("TIMELEX_GATEWAY_KEY_ID").unwrap_or_else(|_| "k1".into());
        let signer =
            Signer::from_base64(&kid, &var("TIMELEX_GATEWAY_SIGNING_KEY")?).map_err(|e| e.to_string())?;
        let service_urls = parse_service_urls(
            &std::env::var("TIMELEX_SERVICE_URLS")
                .unwrap_or_else(|_| "identity=http://127.0.0.1:8081".into()),
        )?;
        let budget = |name: &str, default: u32| match std::env::var(name) {
            Ok(v) => parse_budget(name, &v),
            Err(_) => Ok(default),
        };
        let internal_addr = match std::env::var("TIMELEX_GATEWAY_INTERNAL_ADDR") {
            Ok(v) => parse_addr("TIMELEX_GATEWAY_INTERNAL_ADDR", &v)?,
            Err(_) => DEFAULT_INTERNAL_ADDR,
        };
        Ok(Self {
            signer,
            operator_key_hashes: parse_operator_hashes(&var("TIMELEX_OPERATOR_KEY_SHA256")?)?,
            service_urls,
            upstream_timeout: DEFAULT_UPSTREAM_TIMEOUT,
            rate_limit_per_minute: budget("TIMELEX_RATE_LIMIT_PER_MINUTE", DEFAULT_RATE_LIMIT_PER_MINUTE)?,
            rate_limit_burst: budget("TIMELEX_RATE_LIMIT_BURST", DEFAULT_RATE_LIMIT_BURST)?,
            internal_addr,
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

    #[test]
    fn service_urls_parse_and_trailing_slashes_are_stripped() {
        let urls =
            parse_service_urls(" identity=http://127.0.0.1:8081/ , vault=https://vault.example/ ").unwrap();
        assert_eq!(urls["identity"], "http://127.0.0.1:8081");
        assert_eq!(urls["vault"], "https://vault.example");
        assert_eq!(
            parse_service_urls("identity=http://h").unwrap()["identity"],
            "http://h"
        );
        // A path prefix is kept.
        assert_eq!(
            parse_service_urls("identity=http://h/base/").unwrap()["identity"],
            "http://h/base"
        );
    }

    #[test]
    fn bad_service_urls_are_refused() {
        for bad in [
            "",
            "vault=http://127.0.0.1:8082",         // no identity
            "identity",                            // not a pair
            "identity=",                           // no URL
            "identity=ftp://127.0.0.1",            // wrong scheme
            "identity=http://",                    // no host
            "identity=http://h/?x=1",              // query string
            "identity=http://h/#x",                // fragment
            "identity=http://h,identity=http://i", // twice
            "Identity=http://h",                   // not a lowercase name
            "=http://h",                           // no name
        ] {
            assert!(parse_service_urls(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn budgets_are_positive_whole_numbers_with_a_ceiling() {
        assert_eq!(parse_budget("X", "600").unwrap(), 600);
        assert_eq!(parse_budget("X", " 1 ").unwrap(), 1);
        for bad in ["", "0", "-1", "abc", "1.5", "1000001"] {
            assert!(parse_budget("X", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn internal_addresses_are_parsed_or_refused() {
        assert_eq!(
            parse_addr("X", "127.0.0.1:8089").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], 8089))
        );
        for bad in ["", "127.0.0.1", "localhost:http", "::1:8089"] {
            assert!(parse_addr("X", bad).is_err(), "{bad:?}");
        }
    }
}
