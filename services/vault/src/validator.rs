// Written by Kone Tshivhinda & Claude
//! The provider check behind `POST /v1/secrets/{id}/test`.
//!
//! A secret is opened in this process and handed to a [`ProviderValidator`]. A live validator asks
//! the provider; a test validator answers from a script. The result is only ever a verdict: the
//! provider's own words are not relayed, so nothing it says can end up in a log, an error body or a
//! support ticket.
//!
//! `anthropic_api_key` is the one kind checked against the real provider in 0.1.0: a read-only
//! `GET /v1/models` with the key in a header. Other kinds are accepted once their shape is valid
//! (the store has already enforced the shape), as `contracts/openapi/vault.v1.yaml` says.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use timelex_vault::SecretKind;

use crate::store::LoadedSecret;

/// Read-only endpoint used to check an Anthropic key. Listing one model says nothing about usage.
pub const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models?limit=1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const TIMEOUT: Duration = Duration::from_secs(10);

/// What a provider check found. Deliberately coarser than HTTP: no status, body or message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    /// The provider accepted the credential.
    Accepted,
    /// The provider said the credential is not valid.
    Rejected,
    /// The provider was not reached, or answered in a way that says nothing about the credential.
    Unreachable,
}

/// Where a provider check happens. Injected so the HTTP layer can be tested without a network.
#[async_trait]
pub trait ProviderValidator: Send + Sync {
    /// Check one opened secret. The value is borrowed, never stored or copied out.
    async fn check(&self, secret: &LoadedSecret) -> CheckResult;
}

/// The validator the service runs: real calls for Anthropic, shape-only for the rest.
pub struct LiveValidator {
    client: Client,
    anthropic_url: String,
}

impl LiveValidator {
    /// Build a validator with a client that gives up after [`TIMEOUT`].
    pub fn new() -> Result<Self, String> {
        Self::at(ANTHROPIC_MODELS_URL)
    }

    fn at(anthropic_url: &str) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(TIMEOUT)
            .user_agent("timelex-vault/0.1")
            .build()
            .map_err(|e| format!("cannot build the provider HTTP client: {e}"))?;
        Ok(Self {
            client,
            anthropic_url: anthropic_url.to_owned(),
        })
    }
}

#[async_trait]
impl ProviderValidator for LiveValidator {
    async fn check(&self, secret: &LoadedSecret) -> CheckResult {
        match secret.kind {
            SecretKind::AnthropicApiKey => self.anthropic(secret).await,
            // Shape was checked when the secret was stored; nothing else can be checked without a
            // provider call this version does not make.
            _ => CheckResult::Accepted,
        }
    }
}

impl LiveValidator {
    async fn anthropic(&self, secret: &LoadedSecret) -> CheckResult {
        // The key travels in a header, never in the URL: URLs are the first thing proxies log.
        let response = self
            .client
            .get(&self.anthropic_url)
            .header("x-api-key", secret.value.expose_secret())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await;
        match response {
            Ok(r) => verdict(r.status().as_u16()),
            Err(_) => CheckResult::Unreachable,
        }
    }
}

/// Only a 2xx means the key works, and only 401/403 means it does not. A rate limit, a 5xx or a
/// 3xx is the provider's problem, not the credential's, so it is not reported as a rejection.
fn verdict(status: u16) -> CheckResult {
    match status {
        200..=299 => CheckResult::Accepted,
        401 | 403 => CheckResult::Rejected,
        _ => CheckResult::Unreachable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use timelex_vault::{SecretId, SecretValue};
    use uuid::Uuid;

    /// An opened secret and the value it holds, built at run time so no credential-shaped literal
    /// sits in this file.
    fn opened(kind: SecretKind) -> (LoadedSecret, String) {
        let value = format!("sk-ant-api03-{}", Uuid::new_v4().simple());
        (
            LoadedSecret {
                id: SecretId::from_uuid(Uuid::from_u128(1)),
                kind,
                label: "Firm Anthropic key".into(),
                value: SecretValue::parse(&value).unwrap(),
            },
            value,
        )
    }

    #[test]
    fn only_a_success_is_acceptance_and_only_401_or_403_is_rejection() {
        assert_eq!(verdict(200), CheckResult::Accepted);
        assert_eq!(verdict(401), CheckResult::Rejected);
        assert_eq!(verdict(403), CheckResult::Rejected);
        assert_eq!(verdict(429), CheckResult::Unreachable);
        assert_eq!(verdict(500), CheckResult::Unreachable);
    }

    #[tokio::test]
    async fn kinds_without_a_provider_call_are_accepted_once_stored() {
        let v = LiveValidator::at("http://127.0.0.1:9/never").unwrap();
        for kind in [SecretKind::GenericApiKey, SecretKind::M365Oauth] {
            let (secret, _) = opened(kind);
            assert_eq!(v.check(&secret).await, CheckResult::Accepted);
        }
    }

    /// A local stand-in for Anthropic: records the headers it saw and answers with `status`.
    async fn stub(status: StatusCode) -> (String, Arc<Mutex<HeaderMap>>) {
        let seen = Arc::new(Mutex::new(HeaderMap::new()));
        let recorded = Arc::clone(&seen);
        let app = Router::new().route(
            "/v1/models",
            get(move |headers: HeaderMap| {
                let recorded = Arc::clone(&recorded);
                async move {
                    *recorded.lock().unwrap() = headers;
                    (status, "{}")
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/v1/models?limit=1"), seen)
    }

    #[tokio::test]
    async fn an_anthropic_key_travels_in_a_header_and_a_200_is_accepted() {
        let (url, seen) = stub(StatusCode::OK).await;
        let v = LiveValidator::at(&url).unwrap();
        let (secret, value) = opened(SecretKind::AnthropicApiKey);
        assert_eq!(v.check(&secret).await, CheckResult::Accepted);
        let headers = seen.lock().unwrap();
        assert_eq!(headers["x-api-key"], value);
        assert_eq!(headers["anthropic-version"], ANTHROPIC_VERSION);
    }

    #[tokio::test]
    async fn a_refused_key_is_a_rejection_and_a_provider_failure_is_not() {
        let (url, _) = stub(StatusCode::UNAUTHORIZED).await;
        let v = LiveValidator::at(&url).unwrap();
        let (secret, _) = opened(SecretKind::AnthropicApiKey);
        assert_eq!(v.check(&secret).await, CheckResult::Rejected);

        let (url, _) = stub(StatusCode::SERVICE_UNAVAILABLE).await;
        let v = LiveValidator::at(&url).unwrap();
        let (secret, _) = opened(SecretKind::AnthropicApiKey);
        assert_eq!(v.check(&secret).await, CheckResult::Unreachable);
    }

    #[tokio::test]
    async fn a_provider_that_is_not_there_is_unreachable() {
        // Port 9 (discard) on loopback refuses connections in every environment we test on.
        let v = LiveValidator::at("http://127.0.0.1:9/v1/models").unwrap();
        let (secret, _) = opened(SecretKind::AnthropicApiKey);
        assert_eq!(v.check(&secret).await, CheckResult::Unreachable);
    }
}
