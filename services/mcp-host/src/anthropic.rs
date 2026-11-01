// Written by Kone Tshivhinda & Claude
//! The Anthropic adapter: one `POST /v1/messages` per call, with the tenant's leased key in a
//! header and no provider message in any error.
//!
//! The mapping is chosen for what the loop does next:
//! - `401` is the key itself, so every rung would fail: stop and ask a human.
//! - `403`, `404` and `400` are how a workspace without access to a model answers: try the next
//!   rung. A genuine bad request also lands here; degrading and then asking a human is safe.
//! - `408`, `429`, `5xx` and transport failures are the provider's problem: stop, because moving
//!   to another model will not help a rate limit or an outage.
//! - Any other status, a malformed body, or a reply with no text is unusable: stop.
//!
//! There is no retry policy here. Retries belong to the workflow layer, where backoff can be
//! counted against the same budget.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use timelex_vault::SecretValue;

use crate::provider::{
    Completion, CompletionRequest, ModelCatalog, ModelError, ModelProvider, StopReason, Usage,
};

/// Where the real provider lives.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

const ANTHROPIC_VERSION: &str = "2023-06-01";
const TIMEOUT: Duration = Duration::from_secs(30);

/// A model provider for one tenant's leased Anthropic key. Dropping it wipes the key.
pub struct AnthropicProvider {
    client: Client,
    base_url: String,
    key: SecretValue,
    catalog: ModelCatalog,
}

impl AnthropicProvider {
    /// Build the adapter. The key is the plaintext from a vault lease and lives only as long as
    /// this value.
    pub fn new(base_url: &str, key: SecretValue) -> Result<Self, String> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err("the Anthropic base URL must start with http:// or https://".into());
        }
        let client = Client::builder()
            .timeout(TIMEOUT)
            .user_agent("timelex-mcp-host/0.1")
            .build()
            .map_err(|e| format!("cannot build the model HTTP client: {e}"))?;
        Ok(Self {
            client,
            base_url,
            key,
            catalog: ModelCatalog::default(),
        })
    }

    /// Use different model ids.
    pub fn with_catalog(mut self, catalog: ModelCatalog) -> Self {
        self.catalog = catalog;
        self
    }

    async fn post(&self, request: &CompletionRequest) -> Result<reqwest::Response, ModelError> {
        let payload = serde_json::json!({
            "model": self.catalog.id(request.model),
            "max_tokens": request.max_tokens,
            "system": request.system,
            "messages": [{"role": "user", "content": request.prompt}],
        });
        let body = serde_json::to_vec(&payload).map_err(|_| ModelError::Unusable)?;
        // The key travels in a header, never in the URL or the body: URLs are logged first.
        self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", self.key.expose_secret())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| ModelError::Unreachable)
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ModelError> {
        let response = self.post(&request).await?;
        match response.status().as_u16() {
            200..=299 => parse_reply(response).await,
            401 => Err(ModelError::KeyRejected),
            400 | 403 | 404 => Err(ModelError::ModelUnavailable),
            408 | 429 | 500..=599 => Err(ModelError::Unreachable),
            _ => Err(ModelError::Unusable),
        }
    }
}

/// Read the reply. The body is parsed, never logged, and a provider message is never attached to
/// an error.
async fn parse_reply(response: reqwest::Response) -> Result<Completion, ModelError> {
    let body = response.text().await.map_err(|_| ModelError::Unusable)?;
    let reply: Reply = serde_json::from_str(&body).map_err(|_| ModelError::Unusable)?;
    let text = reply
        .content
        .into_iter()
        .filter(|block| block.kind == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        return Err(ModelError::Unusable);
    }
    Ok(Completion {
        text,
        usage: Usage {
            input_tokens: reply.usage.input_tokens,
            output_tokens: reply.usage.output_tokens,
            cache_read_tokens: reply.usage.cache_read_input_tokens,
            cache_write_tokens: reply.usage.cache_creation_input_tokens,
        },
        stop_reason: match reply.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("max_tokens") => StopReason::MaxTokens,
            _ => StopReason::Other,
        },
    })
}

#[derive(Deserialize)]
struct Reply {
    content: Vec<Block>,
    usage: ReplyUsage,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize, Default)]
struct ReplyUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}
