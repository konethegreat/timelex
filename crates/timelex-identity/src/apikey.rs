// Written by Kone Tshivhinda & Claude
//! API-key tokens and the check for whether a key may be used.
//!
//! A token looks like `tlx_<tenant id as 32 hex characters>.<43 characters of secret>`.
//! * `tlx_` marks it for secret scanners.
//! * The tenant id tells the service which firm's rows to look in *before* it has authenticated
//!   anything. It is a claim, not proof: the hash of the whole token must match a key inside that
//!   firm (row-level security enforces the "inside").
//! * The secret is 32 random bytes written as base64url, 43 characters, no padding.
//!
//! Only the SHA-256 hash is stored. SHA-256 (rather than a slow password hash) is right here because
//! the secret is 256 random bits: there is nothing to guess, so slowing the hash down buys nothing.
//!
//! This crate never creates randomness. The service passes in 32 random bytes from the operating
//! system, which keeps everything here deterministic and testable.

use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::error::{AuthFailure, TokenError};
use crate::ids::TenantId;
use crate::status::{TenantStatus, UserStatus};

const PREFIX: &str = "tlx_";
const TENANT_LEN: usize = 32;
const SECRET_LEN: usize = 43; // 32 bytes in base64url without padding
const TOKEN_LEN: usize = PREFIX.len() + TENANT_LEN + 1 + SECRET_LEN;

/// A complete API-key token. Treat as a password: it has no `Display`, and `Debug` hides it, so it
/// cannot reach a log by accident. Call [`ApiKeyToken::expose_secret`] on purpose, once, to show a
/// new key to its creator.
pub struct ApiKeyToken {
    text: String,
    tenant: TenantId,
}

impl ApiKeyToken {
    /// Assemble a new token from a tenant and 32 random bytes.
    pub fn new(tenant: TenantId, secret: [u8; 32]) -> Self {
        let text = format!("{PREFIX}{}.{}", tenant.to_hex32(), URL_SAFE_NO_PAD.encode(secret));
        debug_assert_eq!(text.len(), TOKEN_LEN);
        Self { text, tenant }
    }

    /// Parse a presented token. Strict: exactly one valid spelling per key, so two different
    /// strings can never mean the same key.
    pub fn parse(s: &str) -> Result<Self, TokenError> {
        if s.len() != TOKEN_LEN {
            return Err(TokenError::BadLength);
        }
        let rest = s.strip_prefix(PREFIX).ok_or(TokenError::BadPrefix)?;
        // `get` returns None instead of panicking if a multi-byte character straddles the split.
        let tenant_hex = rest.get(..TENANT_LEN).ok_or(TokenError::BadTenant)?;
        let tenant = TenantId::from_hex32(tenant_hex).ok_or(TokenError::BadTenant)?;
        let after = rest.get(TENANT_LEN..).ok_or(TokenError::BadSecret)?;
        let secret_b64 = after.strip_prefix('.').ok_or(TokenError::BadSecret)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(secret_b64)
            .map_err(|_| TokenError::BadSecret)?;
        // Reject non-canonical spellings (stray trailing bits) by re-encoding.
        if bytes.len() != 32 || URL_SAFE_NO_PAD.encode(&bytes) != secret_b64 {
            return Err(TokenError::BadSecret);
        }
        Ok(Self {
            text: s.to_owned(),
            tenant,
        })
    }

    /// The firm this token claims to belong to.
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// SHA-256 of the whole token text: what the database stores and looks up.
    pub fn hash(&self) -> KeyHash {
        KeyHash(Sha256::digest(self.text.as_bytes()).into())
    }

    /// First 8 characters of the secret part, safe to show in lists so an admin can tell keys apart.
    pub fn display_prefix(&self) -> &str {
        &self.text[PREFIX.len() + TENANT_LEN + 1..][..8]
    }

    /// The full token. Deliberately awkward to call: only for handing a new key to its creator.
    pub fn expose_secret(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for ApiKeyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKeyToken(tenant={}, secret=****)", self.tenant)
    }
}

/// The 32-byte hash of a token.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyHash([u8; 32]);

impl KeyHash {
    /// Wrap bytes read from the database. Returns `None` unless there are exactly 32.
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        <[u8; 32]>::try_from(b).ok().map(Self)
    }

    /// The bytes, for a database parameter.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Compare without stopping at the first difference, so timing does not reveal how many leading
    /// bytes matched. (The database lookup already matched the hash; this is a second, cheap check.)
    pub fn matches(&self, other: &KeyHash) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

impl fmt::Debug for KeyHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyHash(..)")
    }
}

/// Everything needed to decide whether a stored key may be used right now.
#[derive(Debug, Clone, Copy)]
pub struct KeyState {
    /// When the key expires, if ever.
    pub expires_at: Option<OffsetDateTime>,
    /// When the key was revoked, if it was.
    pub revoked_at: Option<OffsetDateTime>,
    /// Status of the user the key belongs to.
    pub user_status: UserStatus,
    /// Status of the firm.
    pub tenant_status: TenantStatus,
}

/// Decide whether a key may authenticate at `now`. The service reports every failure to the caller
/// as the same 401; the variant is for the audit log.
///
/// A key expires *at* its expiry instant (`now >= expires_at` is expired).
pub fn check_key_usable(state: &KeyState, now: OffsetDateTime) -> Result<(), AuthFailure> {
    if state.tenant_status != TenantStatus::Active {
        return Err(AuthFailure::TenantSuspended);
    }
    if state.revoked_at.is_some() {
        return Err(AuthFailure::Revoked);
    }
    if state.user_status != UserStatus::Active {
        return Err(AuthFailure::UserNotActive);
    }
    if let Some(expires_at) = state.expires_at {
        if now >= expires_at {
            return Err(AuthFailure::Expired);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;
    use uuid::Uuid;

    fn tenant() -> TenantId {
        TenantId::from_uuid(Uuid::parse_str("0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b").unwrap())
    }

    fn token() -> ApiKeyToken {
        ApiKeyToken::new(tenant(), [7u8; 32])
    }

    #[test]
    fn token_has_the_documented_shape() {
        let t = token();
        let s = t.expose_secret();
        assert_eq!(s.len(), 80);
        assert!(s.starts_with("tlx_0191a2b3c4d57e6f8a9b0c1d2e3f4a5b."));
        assert_eq!(t.display_prefix().len(), 8);
    }

    #[test]
    fn token_round_trips_through_parse() {
        let t = token();
        let parsed = ApiKeyToken::parse(t.expose_secret()).unwrap();
        assert_eq!(parsed.tenant(), tenant());
        assert!(parsed.hash().matches(&t.hash()));
    }

    #[test]
    fn different_secrets_give_different_hashes() {
        let a = ApiKeyToken::new(tenant(), [1u8; 32]);
        let b = ApiKeyToken::new(tenant(), [2u8; 32]);
        assert!(!a.hash().matches(&b.hash()));
    }

    #[test]
    fn same_secret_in_another_tenant_is_a_different_key() {
        let other = TenantId::from_uuid(Uuid::parse_str("0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5c").unwrap());
        let a = ApiKeyToken::new(tenant(), [9u8; 32]);
        let b = ApiKeyToken::new(other, [9u8; 32]);
        assert!(!a.hash().matches(&b.hash()));
    }

    #[test]
    fn hash_matches_a_known_answer() {
        // Known-answer test: the token and its SHA-256 were computed outside this crate (with .NET's
        // SHA256 over the ASCII text). If this fails, what is hashed has changed, which would
        // silently invalidate every stored key.
        let t = token();
        assert_eq!(
            t.expose_secret(),
            "tlx_0191a2b3c4d57e6f8a9b0c1d2e3f4a5b.BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc"
        );
        let hex: String = t.hash().as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "b26aea67a66b01ab665b784f157c0c7f52cb7cc1cb5db7c61bb4bf8a1a37779d"
        );
    }

    #[test]
    fn parse_rejects_malformed_tokens() {
        let good = token().expose_secret().to_owned();
        let bad_prefix = good.replacen("tlx_", "tlz_", 1);
        let upper_tenant = good.replacen("0191a2b3", "0191A2B3", 1);
        let no_dot = good.replacen('.', "_", 1);
        let short = good[..79].to_owned();
        let long = format!("{good}A");
        let non_base64 = format!("{}!", &good[..79]);
        // 32 bytes are 256 bits but 43 base64 characters hold 258, so the last character has 2 unused
        // low bits. For this test key it is 'c' (value 28, low bits 00); 'd' (29) sets an unused bit,
        // so a lenient decoder would accept it as the same bytes. A strict parser must not.
        assert!(good.ends_with('c'));
        let non_canonical = format!("{}d", &good[..79]);
        let cases = [
            (bad_prefix, TokenError::BadPrefix),
            (upper_tenant, TokenError::BadTenant),
            (no_dot, TokenError::BadSecret),
            (short, TokenError::BadLength),
            (long, TokenError::BadLength),
            (non_base64, TokenError::BadSecret),
            (String::new(), TokenError::BadLength),
            (non_canonical, TokenError::BadSecret),
        ];
        for (text, expected) in cases {
            assert_eq!(ApiKeyToken::parse(&text).err(), Some(expected), "{text}");
        }
    }

    #[test]
    fn parse_does_not_panic_on_multibyte_input() {
        // 80 bytes but with multi-byte characters where the tenant part would split.
        let s = format!("tlx_{}{}", "é".repeat(16), ".".repeat(44));
        assert_eq!(s.len(), 4 + 32 + 44);
        assert!(ApiKeyToken::parse(&s).is_err());
    }

    #[test]
    fn debug_and_hash_formatting_do_not_leak_the_secret() {
        let t = token();
        let secret_part = t.expose_secret().split('.').nth(1).unwrap().to_owned();
        let shown = format!("{t:?} {:?}", t.hash());
        assert!(!shown.contains(&secret_part));
        assert!(shown.contains("****"));
    }

    #[test]
    fn key_hash_from_bytes_requires_exactly_32() {
        assert!(KeyHash::from_bytes(&[0u8; 31]).is_none());
        assert!(KeyHash::from_bytes(&[0u8; 33]).is_none());
        assert!(KeyHash::from_bytes(&[0u8; 32]).is_some());
    }

    fn state() -> KeyState {
        KeyState {
            expires_at: None,
            revoked_at: None,
            user_status: UserStatus::Active,
            tenant_status: TenantStatus::Active,
        }
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    #[test]
    fn healthy_key_is_usable() {
        assert_eq!(check_key_usable(&state(), now()), Ok(()));
    }

    #[test]
    fn suspended_tenant_blocks_every_key() {
        let s = KeyState {
            tenant_status: TenantStatus::Suspended,
            ..state()
        };
        assert_eq!(check_key_usable(&s, now()), Err(AuthFailure::TenantSuspended));
    }

    #[test]
    fn revoked_key_is_rejected() {
        let s = KeyState {
            revoked_at: Some(now() - Duration::hours(1)),
            ..state()
        };
        assert_eq!(check_key_usable(&s, now()), Err(AuthFailure::Revoked));
    }

    #[test]
    fn invited_or_disabled_owner_blocks_the_key() {
        for status in [UserStatus::Invited, UserStatus::Disabled] {
            let s = KeyState {
                user_status: status,
                ..state()
            };
            assert_eq!(check_key_usable(&s, now()), Err(AuthFailure::UserNotActive));
        }
    }

    #[test]
    fn key_expires_at_its_expiry_instant() {
        let before = KeyState {
            expires_at: Some(now() + Duration::seconds(1)),
            ..state()
        };
        let at = KeyState {
            expires_at: Some(now()),
            ..state()
        };
        let after = KeyState {
            expires_at: Some(now() - Duration::seconds(1)),
            ..state()
        };
        assert_eq!(check_key_usable(&before, now()), Ok(()));
        assert_eq!(check_key_usable(&at, now()), Err(AuthFailure::Expired));
        assert_eq!(check_key_usable(&after, now()), Err(AuthFailure::Expired));
    }

    #[test]
    fn suspension_is_reported_before_other_problems() {
        let s = KeyState {
            tenant_status: TenantStatus::Suspended,
            revoked_at: Some(now()),
            user_status: UserStatus::Disabled,
            expires_at: Some(now() - Duration::days(1)),
        };
        assert_eq!(check_key_usable(&s, now()), Err(AuthFailure::TenantSuspended));
    }
}
