// Written by Kone Tshivhinda & Claude
//! The internal token: how a TimeLex service knows a request came through the gateway, and for whom.
//!
//! Decision and reasons: ADR-0012. Wire format: `contracts/internal-auth.md`. In short: the gateway
//! signs a 30-second EdDSA JWT; a service holds only the *public* key, so it can check tokens but never
//! make one. The service-side check is [`Verifier::verify`]; every failure is reported to the caller as
//! the same 401, and the specific [`VerifyError`] is for the audit log only.
//!
//! This crate does no network I/O. "Now" is a parameter, so every rule is testable with fixed times.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use uuid::Uuid;

/// The only issuer a service accepts.
pub const ISSUER: &str = "timelex-gateway";
/// How long a freshly signed token lives.
pub const TOKEN_LIFETIME_SECONDS: i64 = 30;
/// A verifier rejects any token claiming to live longer than this, even if correctly signed.
pub const MAX_LIFETIME_SECONDS: i64 = 60;
/// Allowed clock difference between gateway and service.
pub const CLOCK_SKEW_SECONDS: i64 = 5;

/// A tenant user or machine, as established by the gateway through `identity.authenticate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The firm.
    pub tenant: TenantId,
    /// The user the API key acts as.
    pub user: UserId,
    /// The API key used.
    pub key: ApiKeyId,
    /// What the key may do right now (its scopes limited by the owner's roles).
    pub permissions: BTreeSet<Permission>,
}

/// Who is calling a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// The gateway itself, for internal calls such as `authenticate`.
    Service,
    /// A platform operator, for operator-only calls such as `createTenant`.
    Operator,
    /// A tenant user or machine.
    Principal(Principal),
}

/// The three kinds of caller, for routes that declare which they accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// See [`Caller::Service`].
    Service,
    /// See [`Caller::Operator`].
    Operator,
    /// See [`Caller::Principal`].
    Principal,
}

impl Caller {
    /// Which kind of caller this is.
    pub fn kind(&self) -> Kind {
        match self {
            Caller::Service => Kind::Service,
            Caller::Operator => Kind::Operator,
            Caller::Principal(_) => Kind::Principal,
        }
    }
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Service => "service",
            Kind::Operator => "operator",
            Kind::Principal => "principal",
        }
    }
}

// The JWT payload. `deny_unknown_fields`: a token with a claim we do not know is rejected, not ignored.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    iss: String,
    aud: String,
    iat: i64,
    exp: i64,
    jti: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tid: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    perms: Option<Vec<String>>,
}

/// A key could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyError(pub String);

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key configuration: {}", self.0)
    }
}
impl std::error::Error for KeyError {}

/// Why a token was refused. Services answer every one of these with the same 401.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// Not three base64url parts, bad header, or the payload is not the expected JSON.
    Malformed,
    /// The header names no `kid`, or a `kid` this service does not trust.
    UnknownKey,
    /// The header algorithm is not `EdDSA`, or `typ` is not `JWT`.
    BadAlgorithm,
    /// The signature does not match.
    BadSignature,
    /// `iss` is not `timelex-gateway`.
    WrongIssuer,
    /// `aud` is not this service.
    WrongAudience,
    /// `exp` has passed (beyond the allowed skew).
    Expired,
    /// `iat` is in the future (beyond the allowed skew).
    NotYetValid,
    /// `exp - iat` is longer than allowed, or not positive.
    BadLifetime,
    /// The claims do not fit the declared `kind`, or an id or permission is malformed.
    BadClaims,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "internal token rejected: {self:?}")
    }
}
impl std::error::Error for VerifyError {}

/// A token that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// Who is calling.
    pub caller: Caller,
    /// The token's unique id (for the audit log).
    pub jti: String,
    /// When the token expires.
    pub expires_at: OffsetDateTime,
}

/// The gateway's signing side. Holds the private key: construct it in the gateway only.
pub struct Signer {
    kid: String,
    key: EncodingKey,
}

impl fmt::Debug for Signer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signer(kid={}, key=****)", self.kid)
    }
}

impl Signer {
    /// Load a private key: base64 of a PKCS#8 DER Ed25519 key, as printed by [`generate_keypair`].
    pub fn from_base64(kid: &str, pkcs8_b64: &str) -> Result<Self, KeyError> {
        if kid.is_empty()
            || kid.len() > 64
            || !kid
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(KeyError("kid must be 1 to 64 letters, digits, '-' or '_'".into()));
        }
        let der = STANDARD
            .decode(pkcs8_b64.trim())
            .map_err(|_| KeyError("private key is not valid base64".into()))?;
        // Parse it ourselves once so a bad key fails at start-up, not on the first request.
        ed25519_dalek::pkcs8::DecodePrivateKey::from_pkcs8_der(&der)
            .map(|_: SigningKey| ())
            .map_err(|_| KeyError("private key is not a PKCS#8 Ed25519 key".into()))?;
        Ok(Self {
            kid: kid.to_owned(),
            key: EncodingKey::from_ed_der(&der),
        })
    }

    /// The key id placed in every token header.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// Sign a token for `audience` (a service name such as `identity`) at time `now`.
    pub fn sign_at(&self, audience: &str, caller: &Caller, now: OffsetDateTime) -> Result<String, KeyError> {
        let iat = now.unix_timestamp();
        let mut claims = Claims {
            iss: ISSUER.into(),
            aud: audience.into(),
            iat,
            exp: iat + TOKEN_LIFETIME_SECONDS,
            jti: Uuid::new_v4().to_string(),
            kind: match caller {
                Caller::Service => Kind::Service,
                Caller::Operator => Kind::Operator,
                Caller::Principal(_) => Kind::Principal,
            }
            .as_str()
            .into(),
            tid: None,
            sub: None,
            key: None,
            perms: None,
        };
        if let Caller::Principal(p) = caller {
            claims.tid = Some(p.tenant.as_uuid());
            claims.sub = Some(p.user.as_uuid());
            claims.key = Some(p.key.as_uuid());
            claims.perms = Some(p.permissions.iter().map(|p| p.as_str().to_owned()).collect());
        }
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&header, &claims, &self.key)
            .map_err(|e| KeyError(format!("signing failed: {e}")))
    }

    /// [`Signer::sign_at`] with the system clock.
    pub fn sign(&self, audience: &str, caller: &Caller) -> Result<String, KeyError> {
        self.sign_at(audience, caller, OffsetDateTime::now_utc())
    }
}

/// The service side. Holds public keys only, and the name of the service it protects.
pub struct Verifier {
    audience: String,
    keys: HashMap<String, DecodingKey>,
}

impl fmt::Debug for Verifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Verifier(audience={}, kids={:?})",
            self.audience,
            self.keys.keys().collect::<Vec<_>>()
        )
    }
}

impl Verifier {
    /// Build from `kid=base64,kid2=base64`, each value the base64 of a raw 32-byte Ed25519 public key.
    /// Two entries allow rotation: both are trusted until the old one is removed.
    pub fn from_config(audience: &str, config: &str) -> Result<Self, KeyError> {
        let mut keys = HashMap::new();
        for entry in config.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (kid, b64) = entry
                .split_once('=')
                .ok_or_else(|| KeyError("expected kid=base64".into()))?;
            let raw = STANDARD
                .decode(b64.trim())
                .map_err(|_| KeyError(format!("public key {kid} is not valid base64")))?;
            if raw.len() != 32 {
                return Err(KeyError(format!(
                    "public key {kid} must be 32 bytes, got {}",
                    raw.len()
                )));
            }
            keys.insert(kid.trim().to_owned(), DecodingKey::from_ed_der(&raw));
        }
        if keys.is_empty() {
            return Err(KeyError("no gateway public key configured".into()));
        }
        Ok(Self {
            audience: audience.to_owned(),
            keys,
        })
    }

    /// Check a presented token at time `now`.
    pub fn verify_at(&self, token: &str, now: OffsetDateTime) -> Result<Verified, VerifyError> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| VerifyError::Malformed)?;
        if header.alg != Algorithm::EdDSA || header.typ.as_deref().is_some_and(|t| t != "JWT") {
            return Err(VerifyError::BadAlgorithm);
        }
        let kid = header.kid.as_deref().ok_or(VerifyError::UnknownKey)?;
        let key = self.keys.get(kid).ok_or(VerifyError::UnknownKey)?;

        // Only the signature and structure are checked by the library; every claim rule is below, so
        // all of them use the same `now` and are tested the same way.
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.required_spec_claims.clear();
        let data = jsonwebtoken::decode::<Claims>(token, key, &validation).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => VerifyError::BadSignature,
            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm => VerifyError::BadAlgorithm,
            _ => VerifyError::Malformed,
        })?;
        let c = data.claims;

        if c.iss != ISSUER {
            return Err(VerifyError::WrongIssuer);
        }
        if c.aud != self.audience {
            return Err(VerifyError::WrongAudience);
        }
        let now_s = now.unix_timestamp();
        let lifetime = c.exp.checked_sub(c.iat).ok_or(VerifyError::BadLifetime)?;
        if lifetime <= 0 || lifetime > MAX_LIFETIME_SECONDS {
            return Err(VerifyError::BadLifetime);
        }
        if now_s > c.exp.saturating_add(CLOCK_SKEW_SECONDS) {
            return Err(VerifyError::Expired);
        }
        if c.iat > now_s.saturating_add(CLOCK_SKEW_SECONDS) {
            return Err(VerifyError::NotYetValid);
        }
        let caller = caller_from_claims(&c)?;
        let expires_at = OffsetDateTime::from_unix_timestamp(c.exp).map_err(|_| VerifyError::BadClaims)?;
        Ok(Verified {
            caller,
            jti: c.jti,
            expires_at,
        })
    }

    /// [`Verifier::verify_at`] with the system clock.
    pub fn verify(&self, token: &str) -> Result<Verified, VerifyError> {
        self.verify_at(token, OffsetDateTime::now_utc())
    }
}

fn caller_from_claims(c: &Claims) -> Result<Caller, VerifyError> {
    let has_principal_fields = c.tid.is_some() || c.sub.is_some() || c.key.is_some() || c.perms.is_some();
    match c.kind.as_str() {
        "service" | "operator" => {
            // These kinds carry no tenant, user, key or permissions, so a token cannot be repurposed.
            if has_principal_fields {
                return Err(VerifyError::BadClaims);
            }
            Ok(if c.kind == "service" {
                Caller::Service
            } else {
                Caller::Operator
            })
        }
        "principal" => {
            let (Some(tid), Some(sub), Some(key), Some(perms)) = (c.tid, c.sub, c.key, c.perms.as_ref())
            else {
                return Err(VerifyError::BadClaims);
            };
            let permissions = perms
                .iter()
                .map(|p| Permission::parse(p))
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(|_| VerifyError::BadClaims)?;
            Ok(Caller::Principal(Principal {
                tenant: TenantId::from_uuid(tid),
                user: UserId::from_uuid(sub),
                key: ApiKeyId::from_uuid(key),
                permissions,
            }))
        }
        _ => Err(VerifyError::BadClaims),
    }
}

/// A freshly generated Ed25519 key pair, in the forms the environment variables expect.
pub struct KeyPair {
    /// Base64 of the PKCS#8 DER private key. Secret: for `TIMELEX_GATEWAY_SIGNING_KEY` only.
    pub private_pkcs8_base64: String,
    /// Base64 of the raw 32-byte public key: for `TIMELEX_GATEWAY_PUBLIC_KEYS`.
    pub public_base64: String,
}

/// Generate a key pair from operating-system randomness.
pub fn generate_keypair() -> Result<KeyPair, KeyError> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| KeyError(format!("no randomness: {e}")))?;
    let signing = SigningKey::from_bytes(&seed);
    let der = signing
        .to_pkcs8_der()
        .map_err(|e| KeyError(format!("encoding failed: {e}")))?;
    Ok(KeyPair {
        private_pkcs8_base64: STANDARD.encode(der.as_bytes()),
        public_base64: STANDARD.encode(signing.verifying_key().to_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn t0() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    fn pair() -> (Signer, String) {
        let kp = generate_keypair().unwrap();
        (
            Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
            format!("k1={}", kp.public_base64),
        )
    }

    fn verifier(config: &str) -> Verifier {
        Verifier::from_config("identity", config).unwrap()
    }

    fn principal() -> Principal {
        Principal {
            tenant: TenantId::from_uuid(Uuid::from_u128(1)),
            user: UserId::from_uuid(Uuid::from_u128(2)),
            key: ApiKeyId::from_uuid(Uuid::from_u128(3)),
            permissions: [Permission::KeysManage, Permission::TimeRead]
                .into_iter()
                .collect(),
        }
    }

    // --- crafting tokens by hand, to attack the verifier the way an adversary would ---

    fn b64url(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn payload(kind: &str, aud: &str, iat: i64, exp: i64) -> serde_json::Value {
        serde_json::json!({"iss": ISSUER, "aud": aud, "iat": iat, "exp": exp, "jti": "x", "kind": kind})
    }

    fn forge(header: serde_json::Value, payload: serde_json::Value, signature: &[u8]) -> String {
        format!(
            "{}.{}.{}",
            b64url(header.to_string().as_bytes()),
            b64url(payload.to_string().as_bytes()),
            b64url(signature)
        )
    }

    // --- happy paths ---

    #[test]
    fn each_kind_round_trips() {
        let (signer, cfg) = pair();
        let v = verifier(&cfg);
        for caller in [Caller::Service, Caller::Operator, Caller::Principal(principal())] {
            let token = signer.sign_at("identity", &caller, t0()).unwrap();
            let out = v.verify_at(&token, t0()).unwrap();
            assert_eq!(out.caller, caller);
            assert_eq!(out.expires_at, t0() + Duration::seconds(TOKEN_LIFETIME_SECONDS));
        }
    }

    #[test]
    fn every_token_has_a_unique_id() {
        let (signer, cfg) = pair();
        let v = verifier(&cfg);
        let a = v
            .verify_at(&signer.sign_at("identity", &Caller::Service, t0()).unwrap(), t0())
            .unwrap();
        let b = v
            .verify_at(&signer.sign_at("identity", &Caller::Service, t0()).unwrap(), t0())
            .unwrap();
        assert_ne!(a.jti, b.jti);
    }

    #[test]
    fn rotation_trusts_both_keys_until_one_is_removed() {
        let (s1, c1) = pair();
        let kp2 = generate_keypair().unwrap();
        let s2 = Signer::from_base64("k2", &kp2.private_pkcs8_base64).unwrap();
        let both = verifier(&format!("{c1}, k2={}", kp2.public_base64));
        assert!(both
            .verify_at(&s1.sign_at("identity", &Caller::Service, t0()).unwrap(), t0())
            .is_ok());
        assert!(both
            .verify_at(&s2.sign_at("identity", &Caller::Service, t0()).unwrap(), t0())
            .is_ok());
        let only_new = verifier(&format!("k2={}", kp2.public_base64));
        assert_eq!(
            only_new
                .verify_at(&s1.sign_at("identity", &Caller::Service, t0()).unwrap(), t0())
                .unwrap_err(),
            VerifyError::UnknownKey
        );
    }

    // --- time ---

    #[test]
    fn token_expires_with_a_five_second_grace() {
        let (signer, cfg) = pair();
        let v = verifier(&cfg);
        let token = signer.sign_at("identity", &Caller::Service, t0()).unwrap();
        let exp = t0() + Duration::seconds(TOKEN_LIFETIME_SECONDS);
        assert!(v.verify_at(&token, exp).is_ok());
        assert!(v
            .verify_at(&token, exp + Duration::seconds(CLOCK_SKEW_SECONDS))
            .is_ok());
        assert_eq!(
            v.verify_at(&token, exp + Duration::seconds(CLOCK_SKEW_SECONDS + 1))
                .unwrap_err(),
            VerifyError::Expired
        );
    }

    #[test]
    fn token_from_the_future_is_rejected_beyond_the_skew() {
        let (signer, cfg) = pair();
        let v = verifier(&cfg);
        let token = signer.sign_at("identity", &Caller::Service, t0()).unwrap();
        assert!(v
            .verify_at(&token, t0() - Duration::seconds(CLOCK_SKEW_SECONDS))
            .is_ok());
        assert_eq!(
            v.verify_at(&token, t0() - Duration::seconds(CLOCK_SKEW_SECONDS + 1))
                .unwrap_err(),
            VerifyError::NotYetValid
        );
    }

    // --- audience and issuer ---

    #[test]
    fn a_token_for_another_service_is_rejected() {
        let (signer, cfg) = pair();
        let token = signer.sign_at("vault", &Caller::Service, t0()).unwrap();
        assert_eq!(
            verifier(&cfg).verify_at(&token, t0()).unwrap_err(),
            VerifyError::WrongAudience
        );
    }

    // --- forgery ---

    #[test]
    fn a_token_signed_by_another_key_is_rejected() {
        let (_, cfg) = pair();
        let (attacker, _) = pair();
        let token = attacker.sign_at("identity", &Caller::Operator, t0()).unwrap();
        assert_eq!(
            verifier(&cfg).verify_at(&token, t0()).unwrap_err(),
            VerifyError::BadSignature
        );
    }

    #[test]
    fn an_unknown_key_id_is_rejected() {
        let (_, cfg) = pair();
        let kp = generate_keypair().unwrap();
        let other = Signer::from_base64("k9", &kp.private_pkcs8_base64).unwrap();
        let token = other.sign_at("identity", &Caller::Service, t0()).unwrap();
        assert_eq!(
            verifier(&cfg).verify_at(&token, t0()).unwrap_err(),
            VerifyError::UnknownKey
        );
    }

    #[test]
    fn a_tampered_payload_is_rejected() {
        let (signer, cfg) = pair();
        let token = signer.sign_at("identity", &Caller::Service, t0()).unwrap();
        let mut parts: Vec<String> = token.split('.').map(str::to_owned).collect();
        // Turn the service token into an operator token without the key.
        parts[1] = b64url(
            payload(
                "operator",
                "identity",
                t0().unix_timestamp(),
                t0().unix_timestamp() + 30,
            )
            .to_string()
            .as_bytes(),
        );
        let forged = parts.join(".");
        assert_eq!(
            verifier(&cfg).verify_at(&forged, t0()).unwrap_err(),
            VerifyError::BadSignature
        );
    }

    #[test]
    fn alg_none_is_rejected() {
        let (_, cfg) = pair();
        let t = forge(
            serde_json::json!({"alg": "none", "typ": "JWT", "kid": "k1"}),
            payload(
                "operator",
                "identity",
                t0().unix_timestamp(),
                t0().unix_timestamp() + 30,
            ),
            b"",
        );
        assert!(verifier(&cfg).verify_at(&t, t0()).is_err());
    }

    #[test]
    fn hmac_signed_with_the_public_key_is_rejected() {
        // The classic algorithm-confusion attack: sign with HS256 using the *public* key as the secret.
        let kp = generate_keypair().unwrap();
        let public = STANDARD.decode(&kp.public_base64).unwrap();
        let header = Header {
            kid: Some("k1".into()),
            ..Header::new(Algorithm::HS256)
        };
        let claims = payload(
            "operator",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        );
        let token = jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(&public)).unwrap();
        let v = verifier(&format!("k1={}", kp.public_base64));
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::BadAlgorithm);
    }

    #[test]
    fn garbage_is_malformed() {
        let (_, cfg) = pair();
        let v = verifier(&cfg);
        for t in ["", "abc", "a.b", "a.b.c", "....", "not a token at all"] {
            assert!(v.verify_at(t, t0()).is_err(), "{t:?}");
        }
    }

    // --- claim shape ---

    #[test]
    fn excessive_lifetimes_are_rejected_even_when_validly_signed() {
        // Needs a signer that mints long-lived tokens, so build the JWT directly with the real key.
        let kp = generate_keypair().unwrap();
        let der = STANDARD.decode(&kp.private_pkcs8_base64).unwrap();
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some("k1".into());
        let v = verifier(&format!("k1={}", kp.public_base64));
        let iat = t0().unix_timestamp();
        for (exp, label) in [
            (iat + MAX_LIFETIME_SECONDS + 1, "too long"),
            (iat, "zero"),
            (iat - 5, "negative"),
        ] {
            let token = jsonwebtoken::encode(
                &header,
                &payload("service", "identity", iat, exp),
                &EncodingKey::from_ed_der(&der),
            )
            .unwrap();
            assert_eq!(
                v.verify_at(&token, t0()).unwrap_err(),
                VerifyError::BadLifetime,
                "{label}"
            );
        }
    }

    fn sign_raw(claims: serde_json::Value) -> (String, Verifier) {
        let kp = generate_keypair().unwrap();
        let der = STANDARD.decode(&kp.private_pkcs8_base64).unwrap();
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some("k1".into());
        let token = jsonwebtoken::encode(&header, &claims, &EncodingKey::from_ed_der(&der)).unwrap();
        (token, verifier(&format!("k1={}", kp.public_base64)))
    }

    #[test]
    fn wrong_issuer_is_rejected() {
        let mut p = payload(
            "service",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        );
        p["iss"] = "someone-else".into();
        let (token, v) = sign_raw(p);
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::WrongIssuer);
    }

    #[test]
    fn principal_tokens_need_every_principal_claim() {
        let base = payload(
            "principal",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        );
        let full = |mut p: serde_json::Value| {
            p["tid"] = Uuid::from_u128(1).to_string().into();
            p["sub"] = Uuid::from_u128(2).to_string().into();
            p["key"] = Uuid::from_u128(3).to_string().into();
            p["perms"] = serde_json::json!(["time:read"]);
            p
        };
        // complete: fine
        let (token, v) = sign_raw(full(base.clone()));
        assert!(v.verify_at(&token, t0()).is_ok());
        // each missing claim: rejected
        for missing in ["tid", "sub", "key", "perms"] {
            let mut p = full(base.clone());
            p.as_object_mut().unwrap().remove(missing);
            let (token, v) = sign_raw(p);
            assert_eq!(
                v.verify_at(&token, t0()).unwrap_err(),
                VerifyError::BadClaims,
                "missing {missing}"
            );
        }
    }

    #[test]
    fn service_and_operator_tokens_cannot_carry_a_tenant() {
        for kind in ["service", "operator"] {
            let mut p = payload(
                kind,
                "identity",
                t0().unix_timestamp(),
                t0().unix_timestamp() + 30,
            );
            p["tid"] = Uuid::from_u128(1).to_string().into();
            let (token, v) = sign_raw(p);
            assert_eq!(
                v.verify_at(&token, t0()).unwrap_err(),
                VerifyError::BadClaims,
                "{kind}"
            );
        }
    }

    #[test]
    fn unknown_kind_unknown_permission_and_unknown_claim_are_rejected() {
        let (token, v) = sign_raw(payload(
            "admin",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        ));
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::BadClaims);

        let mut p = payload(
            "principal",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        );
        p["tid"] = Uuid::from_u128(1).to_string().into();
        p["sub"] = Uuid::from_u128(2).to_string().into();
        p["key"] = Uuid::from_u128(3).to_string().into();
        p["perms"] = serde_json::json!(["root:everything"]);
        let (token, v) = sign_raw(p);
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::BadClaims);

        let mut p = payload(
            "service",
            "identity",
            t0().unix_timestamp(),
            t0().unix_timestamp() + 30,
        );
        p["extra"] = "surprise".into();
        let (token, v) = sign_raw(p);
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::Malformed);
    }

    // --- key handling ---

    #[test]
    fn bad_keys_fail_at_load_time() {
        assert!(Signer::from_base64("k1", "not base64!").is_err());
        assert!(
            Signer::from_base64("k1", &STANDARD.encode([0u8; 10])).is_err(),
            "not PKCS#8"
        );
        let kp = generate_keypair().unwrap();
        assert!(
            Signer::from_base64("", &kp.private_pkcs8_base64).is_err(),
            "empty kid"
        );
        assert!(Signer::from_base64("bad kid", &kp.private_pkcs8_base64).is_err());
        assert!(Verifier::from_config("identity", "").is_err(), "no keys");
        assert!(Verifier::from_config("identity", "k1").is_err(), "no '='");
        assert!(
            Verifier::from_config("identity", &format!("k1={}", STANDARD.encode([0u8; 31]))).is_err(),
            "wrong length"
        );
    }

    #[test]
    fn debug_output_hides_the_private_key() {
        let kp = generate_keypair().unwrap();
        let s = Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap();
        let shown = format!("{s:?}");
        assert!(!shown.contains(&kp.private_pkcs8_base64));
        assert!(shown.contains("****"));
    }

    #[test]
    fn generated_keys_differ() {
        let a = generate_keypair().unwrap();
        let b = generate_keypair().unwrap();
        assert_ne!(a.private_pkcs8_base64, b.private_pkcs8_base64);
        assert_ne!(a.public_base64, b.public_base64);
    }
}
