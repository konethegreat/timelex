// Written by Kone Tshivhinda & Claude
//! Workload tokens: how one TimeLex service proves to another who it is, for which tenant, and why.
//!
//! The gateway's internal token ([`crate::Verifier`]) names the *user* a request acts for. A
//! background caller such as `mcp-host` has no user of its own but still needs the vault, so it signs
//! its own token with its own key: issuer `timelex-<service>`, kind `workload`, and a scope made of
//! the tenant, the initiating user and a purpose from a fixed vocabulary. A service that trusts a
//! workload key trusts exactly one service name, so one service's key can never speak for another.
//!
//! Decision and reasons: ADR-0020.

use std::collections::HashMap;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use jsonwebtoken::DecodingKey;
use time::OffsetDateTime;
use timelex_identity::{TenantId, UserId};
use uuid::Uuid;

use crate::{
    check_times, decode_claims, valid_kid, Claims, KeyError, Signer, VerifyError, TOKEN_LIFETIME_SECONDS,
};

/// Every workload issuer starts with this; the rest of the issuer is the service name.
const ISSUER_PREFIX: &str = "timelex-";

/// A service name is short, lowercase and URL-safe, so it can also be an audience later.
fn valid_service(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A purpose is a short lowercase phrase from a fixed vocabulary. The vault decides which purposes
/// may lease which kinds; this only keeps the value log- and audit-safe.
fn valid_purpose(purpose: &str) -> bool {
    !purpose.is_empty()
        && purpose.len() <= 64
        && purpose
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

/// The signing side of a service identity. Holds the private key: construct it in that service only.
#[derive(Debug)]
pub struct WorkloadSigner {
    service: String,
    signer: Signer,
}

impl WorkloadSigner {
    /// `service` is the name this token speaks for (`mcp-host`), and `kid` names the key.
    pub fn from_base64(service: &str, kid: &str, pkcs8_b64: &str) -> Result<Self, KeyError> {
        if !valid_service(service) {
            return Err(KeyError(
                "service must be 1 to 32 lowercase letters, digits or '-'".into(),
            ));
        }
        Ok(Self {
            service: service.to_owned(),
            signer: Signer::from_base64(kid, pkcs8_b64)?,
        })
    }

    /// The service name this signer speaks for.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// Sign a token for `audience` about `tenant`, on behalf of `user`, for `purpose`.
    pub fn sign_at(
        &self,
        audience: &str,
        tenant: TenantId,
        user: UserId,
        purpose: &str,
        now: OffsetDateTime,
    ) -> Result<String, KeyError> {
        if audience.is_empty() {
            return Err(KeyError("an audience is required".into()));
        }
        if !valid_purpose(purpose) {
            return Err(KeyError(
                "purpose must be 1 to 64 lowercase letters, digits, '_', '.', ':' or '-'".into(),
            ));
        }
        let iat = now.unix_timestamp();
        let claims = Claims {
            iss: format!("{ISSUER_PREFIX}{}", self.service),
            aud: audience.to_owned(),
            iat,
            exp: iat + TOKEN_LIFETIME_SECONDS,
            jti: Uuid::new_v4().to_string(),
            kind: "workload".into(),
            tid: Some(tenant.as_uuid()),
            sub: Some(user.as_uuid()),
            key: None,
            perms: None,
            svc: Some(self.service.clone()),
            purpose: Some(purpose.to_owned()),
        };
        self.signer.sign_claims(&claims)
    }

    /// [`WorkloadSigner::sign_at`] with the system clock.
    pub fn sign(
        &self,
        audience: &str,
        tenant: TenantId,
        user: UserId,
        purpose: &str,
    ) -> Result<String, KeyError> {
        self.sign_at(audience, tenant, user, purpose, OffsetDateTime::now_utc())
    }
}

/// A workload token that passed every check. It is scoped, never general: one service, one firm,
/// one initiating user, one purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWorkload {
    /// The service that signed the token.
    pub service: String,
    /// The firm the call is about.
    pub tenant: TenantId,
    /// The user whose work caused the call (the audit actor).
    pub user: UserId,
    /// Why the service is calling, from the caller's fixed vocabulary.
    pub purpose: String,
    /// The token's unique id.
    pub jti: String,
    /// When the token expires.
    pub expires_at: OffsetDateTime,
}

/// The service side. Trusts an explicit service-to-keys map, and nothing else.
pub struct WorkloadVerifier {
    audience: String,
    keys: HashMap<String, (String, DecodingKey)>,
}

impl fmt::Debug for WorkloadVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let services: Vec<&str> = self.keys.values().map(|(service, _)| service.as_str()).collect();
        write!(
            f,
            "WorkloadVerifier(audience={}, services={services:?})",
            self.audience
        )
    }
}

impl WorkloadVerifier {
    /// Build from `service:kid=base64[,kid=base64][;service:...]`, each value the base64 of a raw
    /// 32-byte Ed25519 public key. Key ids must be unique across services, so a token's key alone
    /// names the one service allowed to use it.
    pub fn from_config(audience: &str, config: &str) -> Result<Self, KeyError> {
        if audience.trim().is_empty() {
            return Err(KeyError("an audience is required".into()));
        }
        let mut keys = HashMap::new();
        for entry in config.split(';').map(str::trim).filter(|entry| !entry.is_empty()) {
            let (service, list) = entry
                .split_once(':')
                .ok_or_else(|| KeyError("expected service:kid=base64".into()))?;
            if !valid_service(service) {
                return Err(KeyError(format!(
                    "service {service:?} must be 1 to 32 lowercase letters, digits or '-'"
                )));
            }
            let mut count = 0;
            for pair in list.split(',').map(str::trim).filter(|pair| !pair.is_empty()) {
                let (kid, b64) = pair
                    .split_once('=')
                    .ok_or_else(|| KeyError("expected kid=base64".into()))?;
                if !valid_kid(kid) {
                    return Err(KeyError("kid must be 1 to 64 letters, digits, '-' or '_'".into()));
                }
                let raw = STANDARD
                    .decode(b64.trim())
                    .map_err(|_| KeyError(format!("public key {kid} is not valid base64")))?;
                if raw.len() != 32 {
                    return Err(KeyError(format!(
                        "public key {kid} must be 32 bytes, got {}",
                        raw.len()
                    )));
                }
                if keys.contains_key(kid) {
                    return Err(KeyError(format!(
                        "key id {kid} is already trusted for another service; use distinct ids"
                    )));
                }
                keys.insert(
                    kid.to_owned(),
                    (service.to_owned(), DecodingKey::from_ed_der(&raw)),
                );
                count += 1;
            }
            if count == 0 {
                return Err(KeyError(format!("service {service} has no keys")));
            }
        }
        if keys.is_empty() {
            return Err(KeyError("no workload public key configured".into()));
        }
        Ok(Self {
            audience: audience.to_owned(),
            keys,
        })
    }

    /// The audience this verifier protects.
    pub fn audience(&self) -> &str {
        &self.audience
    }

    /// Check a presented workload token at time `now`.
    pub fn verify_at(&self, token: &str, now: OffsetDateTime) -> Result<VerifiedWorkload, VerifyError> {
        let (c, kid) = decode_claims(token, |kid| self.keys.get(kid).map(|(_, key)| key))?;
        // The kind comes first: a gateway token signed with a key this service happens to trust is
        // still not a workload token, and must not be judged by the issuer rule.
        if c.kind != "workload" {
            return Err(VerifyError::BadClaims);
        }
        let (service, _) = self.keys.get(&kid).ok_or(VerifyError::UnknownKey)?;
        if c.iss != format!("{ISSUER_PREFIX}{service}") {
            return Err(VerifyError::WrongIssuer);
        }
        if c.aud != self.audience {
            return Err(VerifyError::WrongAudience);
        }
        let expires_at = check_times(&c, now)?;
        if c.svc.as_deref() != Some(service.as_str()) || c.key.is_some() || c.perms.is_some() {
            return Err(VerifyError::BadClaims);
        }
        let (Some(tid), Some(sub), Some(purpose)) = (c.tid, c.sub, c.purpose.as_deref()) else {
            return Err(VerifyError::BadClaims);
        };
        if !valid_purpose(purpose) {
            return Err(VerifyError::BadClaims);
        }
        Ok(VerifiedWorkload {
            service: service.clone(),
            tenant: TenantId::from_uuid(tid),
            user: UserId::from_uuid(sub),
            purpose: purpose.to_owned(),
            jti: c.jti,
            expires_at,
        })
    }

    /// [`WorkloadVerifier::verify_at`] with the system clock.
    pub fn verify(&self, token: &str) -> Result<VerifiedWorkload, VerifyError> {
        self.verify_at(token, OffsetDateTime::now_utc())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    use time::{Duration, OffsetDateTime};
    use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
    use uuid::Uuid;

    use crate::{
        generate_keypair, Caller, Principal, Signer, Verifier, VerifyError, WorkloadSigner, WorkloadVerifier,
    };

    fn t0() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    fn tenant() -> TenantId {
        TenantId::from_uuid(Uuid::from_u128(1))
    }

    fn user() -> UserId {
        UserId::from_uuid(Uuid::from_u128(2))
    }

    fn public(kp: &crate::KeyPair) -> String {
        kp.public_base64.clone()
    }

    fn verifier(service: &str, kid: &str, public_b64: &str) -> WorkloadVerifier {
        WorkloadVerifier::from_config("vault", &format!("{service}:{kid}={public_b64}")).unwrap()
    }

    fn signer(service: &str, kid: &str, kp: &crate::KeyPair) -> WorkloadSigner {
        WorkloadSigner::from_base64(service, kid, &kp.private_pkcs8_base64).unwrap()
    }

    /// Sign arbitrary claims with the real key, the way an attacker who stole it would.
    fn sign_raw(kp: &crate::KeyPair, kid: &str, claims: serde_json::Value) -> String {
        let der = STANDARD.decode(&kp.private_pkcs8_base64).unwrap();
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(kid.into());
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_ed_der(&der)).unwrap()
    }

    fn claims(svc: &str, iss: &str, aud: &str) -> serde_json::Value {
        json!({
            "iss": iss,
            "aud": aud,
            "iat": t0().unix_timestamp(),
            "exp": t0().unix_timestamp() + 30,
            "jti": "j1",
            "kind": "workload",
            "svc": svc,
            "tid": tenant().as_uuid().to_string(),
            "sub": user().as_uuid().to_string(),
            "purpose": "model_run"
        })
    }

    #[test]
    fn workload_round_trips_with_tenant_user_and_purpose() {
        let kp = generate_keypair().unwrap();
        let token = signer("mcp-host", "mh1", &kp)
            .sign_at("vault", tenant(), user(), "model_run", t0())
            .unwrap();
        let out = verifier("mcp-host", "mh1", &public(&kp))
            .verify_at(&token, t0())
            .unwrap();
        assert_eq!(out.service, "mcp-host");
        assert_eq!(out.tenant, tenant());
        assert_eq!(out.user, user());
        assert_eq!(out.purpose, "model_run");
        assert_eq!(
            out.expires_at,
            t0() + Duration::seconds(crate::TOKEN_LIFETIME_SECONDS)
        );
    }

    #[test]
    fn workload_token_for_another_audience_is_rejected() {
        let kp = generate_keypair().unwrap();
        let token = signer("mcp-host", "mh1", &kp)
            .sign_at("identity", tenant(), user(), "model_run", t0())
            .unwrap();
        assert_eq!(
            verifier("mcp-host", "mh1", &public(&kp))
                .verify_at(&token, t0())
                .unwrap_err(),
            VerifyError::WrongAudience
        );
    }

    #[test]
    fn workload_issuer_names_the_signing_service() {
        let kp = generate_keypair().unwrap();
        let token = sign_raw(&kp, "mh1", claims("mcp-host", "timelex-capture-hub", "vault"));
        assert_eq!(
            verifier("mcp-host", "mh1", &public(&kp))
                .verify_at(&token, t0())
                .unwrap_err(),
            VerifyError::WrongIssuer
        );
    }

    #[test]
    fn a_workload_token_cannot_claim_another_service() {
        let kp = generate_keypair().unwrap();
        let token = sign_raw(&kp, "mh1", claims("capture-hub", "timelex-mcp-host", "vault"));
        assert_eq!(
            verifier("mcp-host", "mh1", &public(&kp))
                .verify_at(&token, t0())
                .unwrap_err(),
            VerifyError::BadClaims
        );
    }

    #[test]
    fn an_untrusted_key_is_rejected_even_with_the_right_name() {
        let trusted = generate_keypair().unwrap();
        let attacker = generate_keypair().unwrap();
        let token = signer("mcp-host", "mh1", &attacker)
            .sign_at("vault", tenant(), user(), "model_run", t0())
            .unwrap();
        let v = verifier("mcp-host", "mh1", &public(&trusted));
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::BadSignature);
    }

    #[test]
    fn a_gateway_principal_token_is_not_a_workload_token() {
        let kp = generate_keypair().unwrap();
        let gateway = Signer::from_base64("mh1", &kp.private_pkcs8_base64).unwrap();
        let principal = Principal {
            tenant: tenant(),
            user: user(),
            key: ApiKeyId::from_uuid(Uuid::from_u128(3)),
            permissions: [Permission::SettingsManage].into_iter().collect::<BTreeSet<_>>(),
        };
        let token = gateway
            .sign_at("vault", &Caller::Principal(principal), t0())
            .unwrap();
        // The same key is trusted here on purpose: the kind, not the key, must refuse it.
        assert_eq!(
            verifier("mcp-host", "mh1", &public(&kp))
                .verify_at(&token, t0())
                .unwrap_err(),
            VerifyError::BadClaims
        );
    }

    #[test]
    fn workload_tokens_need_every_scope_claim() {
        let kp = generate_keypair().unwrap();
        let v = verifier("mcp-host", "mh1", &public(&kp));
        for missing in ["svc", "tid", "sub", "purpose"] {
            let mut c = claims("mcp-host", "timelex-mcp-host", "vault");
            c.as_object_mut().unwrap().remove(missing);
            let token = sign_raw(&kp, "mh1", c);
            assert_eq!(
                v.verify_at(&token, t0()).unwrap_err(),
                VerifyError::BadClaims,
                "missing {missing}"
            );
        }
    }

    #[test]
    fn workload_tokens_cannot_carry_principal_claims() {
        let kp = generate_keypair().unwrap();
        let v = verifier("mcp-host", "mh1", &public(&kp));
        let extra_claims = [
            ("key", json!(Uuid::from_u128(3).to_string())),
            ("perms", json!(["time:read"])),
        ];
        for (extra, value) in extra_claims {
            let mut c = claims("mcp-host", "timelex-mcp-host", "vault");
            c[extra] = value;
            let token = sign_raw(&kp, "mh1", c);
            assert_eq!(
                v.verify_at(&token, t0()).unwrap_err(),
                VerifyError::BadClaims,
                "{extra}"
            );
        }
    }

    #[test]
    fn workload_lifetime_and_clock_rules_match_the_gateway() {
        let kp = generate_keypair().unwrap();
        let v = verifier("mcp-host", "mh1", &public(&kp));
        let signer = signer("mcp-host", "mh1", &kp);
        let token = signer
            .sign_at("vault", tenant(), user(), "model_run", t0())
            .unwrap();
        let exp = t0() + Duration::seconds(crate::TOKEN_LIFETIME_SECONDS);
        assert!(v
            .verify_at(&token, exp + Duration::seconds(crate::CLOCK_SKEW_SECONDS))
            .is_ok());
        assert_eq!(
            v.verify_at(&token, exp + Duration::seconds(crate::CLOCK_SKEW_SECONDS + 1))
                .unwrap_err(),
            VerifyError::Expired
        );
        assert_eq!(
            v.verify_at(&token, t0() - Duration::seconds(crate::CLOCK_SKEW_SECONDS + 1))
                .unwrap_err(),
            VerifyError::NotYetValid
        );

        let mut long = claims("mcp-host", "timelex-mcp-host", "vault");
        long["exp"] = (t0().unix_timestamp() + crate::MAX_LIFETIME_SECONDS + 1).into();
        let long = sign_raw(&kp, "mh1", long);
        assert_eq!(v.verify_at(&long, t0()).unwrap_err(), VerifyError::BadLifetime);
    }

    #[test]
    fn workload_names_and_purposes_are_restricted() {
        let kp = generate_keypair().unwrap();
        for bad in ["", "Mcp", "-mcp", "mcp host", "mcp_host"] {
            assert!(
                WorkloadSigner::from_base64(bad, "mh1", &kp.private_pkcs8_base64).is_err(),
                "service {bad:?}"
            );
        }
        assert!(
            WorkloadSigner::from_base64(&"a".repeat(33), "mh1", &kp.private_pkcs8_base64).is_err(),
            "a service name is at most 32 characters"
        );

        let s = signer("mcp-host", "mh1", &kp);
        for bad in ["", "Model Run", "MODEL", "a/b", &"p".repeat(65)] {
            assert!(
                s.sign_at("vault", tenant(), user(), bad, t0()).is_err(),
                "purpose {bad:?}"
            );
        }
        assert!(s
            .sign_at("vault", tenant(), user(), "capture.sync-2", t0())
            .is_ok());
    }

    #[test]
    fn workload_config_refuses_malformed_or_ambiguous_keys() {
        let kp = generate_keypair().unwrap();
        let key = public(&kp);
        for bad in [
            "".to_string(),
            "mcp-host".into(),
            "mcp-host:".into(),
            "mcp-host:mh1".into(),
            "mcp-host:mh1=".into(),
            "Mcp-host:mh1={key}".into(),
            format!("mcp-host:mh1={key},mh1={key}"),
            format!("mcp-host:mh1={key};capture-hub:mh1={key}"),
            format!("mcp-host:mh1={}", STANDARD.encode([0u8; 31])),
            format!("mcp-host:bad kid={key}"),
        ] {
            assert!(
                WorkloadVerifier::from_config("vault", &bad).is_err(),
                "accepted {bad:?}"
            );
        }
        let both = format!("mcp-host:mh1={key},mh2={key};capture-hub:ch1={key}");
        let v = WorkloadVerifier::from_config("vault", &both).unwrap();
        assert!(format!("{v:?}").contains("mcp-host"));
    }

    #[test]
    fn gateway_verifier_rejects_workload_claims_on_a_gateway_kind() {
        let kp = generate_keypair().unwrap();
        let mut c = claims("mcp-host", "timelex-gateway", "identity");
        c["kind"] = "service".into();
        let token = sign_raw(&kp, "k1", c);
        let v = Verifier::from_config("identity", &format!("k1={}", public(&kp))).unwrap();
        assert_eq!(v.verify_at(&token, t0()).unwrap_err(), VerifyError::BadClaims);
    }

    #[test]
    fn debug_output_hides_the_workload_private_key() {
        let kp = generate_keypair().unwrap();
        let s = signer("mcp-host", "mh1", &kp);
        let shown = format!("{s:?}");
        assert!(!shown.contains(&kp.private_pkcs8_base64), "{shown}");
        assert!(shown.contains("mcp-host"));
        assert!(shown.contains("****"));
        assert_eq!(s.service(), "mcp-host");
        let v = verifier("mcp-host", "mh1", &public(&kp));
        assert_eq!(v.audience(), "vault");
    }
}
