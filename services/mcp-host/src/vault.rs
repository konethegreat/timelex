// Written by Kone Tshivhinda & Claude
//! Leasing the firm's provider key from the vault (ADR-0004, ADR-0020).
//!
//! This is the host's side of `/internal/v1/secrets/{kind}/lease`. It signs a workload token with
//! its own key, naming the firm, the initiating user and a [`LeasePurpose`], and gets back one copy
//! of the value with an expiry. The value is wrapped in [`SecretValue`] so it is wiped on drop and
//! never printed.
//!
//! The errors are the same closed classes as the model adapter: the status maps to what the caller
//! should do next, and no vault or provider text is ever attached.
//! - `401` → [`LeaseError::Unauthorized`]: this host's key is not trusted, or the vault was started
//!   without one. A human must fix the configuration.
//! - `403` → [`LeaseError::Forbidden`]: the purpose does not grant this kind. A code or contract
//!   error; do not retry.
//! - `404` → [`LeaseError::NotFound`]: the firm has no live key. An admin must add one.
//! - `408`, `429`, `5xx`, transport → [`LeaseError::Unavailable`]: the vault is not answering.
//! - anything else, a malformed body or a kind we did not ask for → [`LeaseError::Unusable`].

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use time::OffsetDateTime;
use timelex_auth::WorkloadSigner;
use timelex_identity::{TenantId, UserId};
use timelex_vault::{SecretKind, SecretValue};

/// The audience a workload token for the vault carries.
const AUDIENCE: &str = "vault";
const TIMEOUT: Duration = Duration::from_secs(10);

/// Why the host is asking for a key. The vault maps each purpose to exactly one kind, so this is
/// also the scope of what a stolen host key could reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeasePurpose {
    /// A model call on the firm's Anthropic key.
    ModelRun,
}

impl LeasePurpose {
    /// The purpose as it appears in the token and the audit entry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelRun => "model_run",
        }
    }

    /// The only secret kind this purpose may lease.
    pub fn kind(self) -> SecretKind {
        match self {
            Self::ModelRun => SecretKind::AnthropicApiKey,
        }
    }
}

/// Every way a lease can fail, chosen for what the caller does next. No variant carries a message:
/// the vault's body and the transport's text stay out of logs and errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    /// The vault does not trust this host's key, or has none configured.
    Unauthorized,
    /// The token's purpose does not grant the requested kind.
    Forbidden,
    /// The firm has no live secret of that kind.
    NotFound,
    /// The vault was not reached, or is throttling or failing.
    Unavailable,
    /// The reply was not a lease we can use.
    Unusable,
}

impl std::fmt::Display for LeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "the vault does not trust this service's key",
            Self::Forbidden => "the vault refused this purpose for this kind",
            Self::NotFound => "the firm has no live secret of this kind",
            Self::Unavailable => "the vault did not answer",
            Self::Unusable => "the vault's answer was not a usable lease",
        })
    }
}

impl std::error::Error for LeaseError {}

/// A client for the vault's internal lease surface, built once per process from this service's key.
pub struct VaultKeySource {
    client: Client,
    base_url: String,
    signer: WorkloadSigner,
}

impl std::fmt::Debug for VaultKeySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultKeySource")
            .field("base_url", &self.base_url)
            .field("service", &self.signer.service())
            .finish_non_exhaustive()
    }
}

impl VaultKeySource {
    /// Build a client. The private key belongs to this service and to nothing else; it is moved in
    /// and wiped on drop.
    pub fn new(base_url: &str, signer: WorkloadSigner) -> Result<Self, String> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err("the vault base URL must start with http:// or https://".into());
        }
        let client = Client::builder()
            .timeout(TIMEOUT)
            .user_agent("timelex-mcp-host/0.1")
            .build()
            .map_err(|e| format!("cannot build the vault HTTP client: {e}"))?;
        Ok(Self {
            client,
            base_url,
            signer,
        })
    }

    /// Lease the firm's live key for `purpose`. `user` is the person whose work caused the call and
    /// becomes the audit actor; the vault records the service and purpose next to it.
    pub async fn lease(
        &self,
        tenant: TenantId,
        user: UserId,
        purpose: LeasePurpose,
    ) -> Result<Lease, LeaseError> {
        let token = self
            .signer
            .sign(AUDIENCE, tenant, user, purpose.as_str())
            .map_err(|_| LeaseError::Unusable)?;
        let response = self
            .client
            .post(format!(
                "{}/internal/v1/secrets/{}/lease",
                self.base_url,
                purpose.kind().as_str()
            ))
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| LeaseError::Unavailable)?;
        match response.status().as_u16() {
            200..=299 => parse_reply(response, purpose.kind()).await,
            401 => Err(LeaseError::Unauthorized),
            403 => Err(LeaseError::Forbidden),
            404 => Err(LeaseError::NotFound),
            408 | 429 | 500..=599 => Err(LeaseError::Unavailable),
            _ => Err(LeaseError::Unusable),
        }
    }
}

/// One copy of the firm's key and the moment it stops being usable. Dropping the lease wipes it.
pub struct Lease {
    kind: SecretKind,
    value: SecretValue,
    expires_at: OffsetDateTime,
}

impl std::fmt::Debug for Lease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lease")
            .field("kind", &self.kind)
            .field("expires_at", &self.expires_at)
            .field("value", &"****")
            .finish()
    }
}

impl Lease {
    /// The kind that was leased.
    pub fn kind(&self) -> SecretKind {
        self.kind
    }

    /// The value, for building a provider. Only the provider boundary may call this.
    pub fn value(&self) -> &SecretValue {
        &self.value
    }

    /// Move the value into a provider, wiping the rest of the lease with it.
    pub fn into_value(self) -> SecretValue {
        self.value
    }

    /// When the caller must stop using the value: at this moment, or when the run ends if sooner.
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseReply {
    kind: String,
    value: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

/// The body is parsed, never logged, and only the kind we asked for is accepted.
async fn parse_reply(response: reqwest::Response, expected: SecretKind) -> Result<Lease, LeaseError> {
    let body = response.text().await.map_err(|_| LeaseError::Unusable)?;
    let reply: LeaseReply = serde_json::from_str(&body).map_err(|_| LeaseError::Unusable)?;
    if SecretKind::parse(&reply.kind) != Some(expected) {
        return Err(LeaseError::Unusable);
    }
    let value = SecretValue::parse(&reply.value).map_err(|_| LeaseError::Unusable)?;
    Ok(Lease {
        kind: expected,
        value,
        expires_at: reply.expires_at,
    })
}
