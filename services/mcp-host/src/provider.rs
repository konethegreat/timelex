// Written by Kone Tshivhinda & Claude
//! What the loop needs from a model, and the four ways a provider can fail.
//!
//! The error set is deliberately about *what the loop does next*: try a weaker model, stop and
//! ask a human, or treat the provider as temporarily absent. Provider messages and status codes do
//! not cross this boundary, so nothing the provider says can reach a log, a prompt or an event.

use async_trait::async_trait;

/// The rungs of the escalation ladder, weakest and cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModelTier {
    Haiku,
    Sonnet,
    Opus,
    Fable,
}

impl ModelTier {
    /// Every tier in ladder order.
    pub const ALL: [Self; 4] = [Self::Haiku, Self::Sonnet, Self::Opus, Self::Fable];

    /// The name used in logs and events.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Haiku => "haiku",
            Self::Sonnet => "sonnet",
            Self::Opus => "opus",
            Self::Fable => "fable",
        }
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Tokens a reply used, including the cache counters the provider reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Why the model stopped. `Other` covers a provider reason this version does not name; it is
/// still a reply the workflow can validate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    Other,
}

/// One model call's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRequest {
    pub model: ModelTier,
    pub system: String,
    pub prompt: String,
    pub max_tokens: u32,
}

/// One model call's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub text: String,
    pub usage: Usage,
    pub stop_reason: StopReason,
}

/// What the loop does when a call fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// The key is fine, but this workspace cannot use this model. Try the next rung.
    ModelUnavailable,
    /// The provider rejected the key itself. Every rung shares it, so stop and ask a human.
    KeyRejected,
    /// The provider was not reached, rate-limited us, or failed. Not the key's fault.
    Unreachable,
    /// A reply arrived but could not be used (malformed, or no text).
    Unusable,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ModelUnavailable => "the model is not available to this key",
            Self::KeyRejected => "the provider rejected the key",
            Self::Unreachable => "the provider could not be reached",
            Self::Unusable => "the provider's reply could not be used",
        })
    }
}

impl std::error::Error for ModelError {}

/// The one way the loop talks to a model. Tests script it; production wraps a provider's HTTP API.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Run one completion, or say why not in the loop's own terms.
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ModelError>;
}

/// The provider's model ids, by tier. Defaults are the aliases our research uses; a deployment
/// can replace every id without touching the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    ids: [String; 4],
}

impl ModelCatalog {
    /// The four ids, in ladder order.
    pub fn new(
        haiku: impl Into<String>,
        sonnet: impl Into<String>,
        opus: impl Into<String>,
        fable: impl Into<String>,
    ) -> Self {
        Self {
            ids: [haiku.into(), sonnet.into(), opus.into(), fable.into()],
        }
    }

    /// The provider id for a tier.
    pub fn id(&self, tier: ModelTier) -> &str {
        &self.ids[tier as usize]
    }
}

impl Default for ModelCatalog {
    fn default() -> Self {
        Self::new(
            "claude-haiku-5-5",
            "claude-sonnet-5-5",
            "claude-opus-5-5",
            "claude-fable-5-1",
        )
    }
}
