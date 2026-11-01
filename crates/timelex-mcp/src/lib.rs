// Written by Kone Tshivhinda & Claude
//! The thin, pinned `rmcp` adapter: the one place that decides how TimeLex speaks MCP.
//!
//! The SDK's defaults are not safe for a stateless, public HTTP deployment (research 08 §5):
//! sessions are on, per-request metadata is optional, Origin validation is off, and every
//! protocol version is accepted. This crate turns those defaults into the configuration
//! [ADR-0015](../../../docs/adr/0015-mcp-adapter.md) fixes, and its contract tests are a Rust
//! port of the P1-08 probe script, so an `rmcp` bump that changes transport behaviour fails CI.

use std::{borrow::Cow, fmt, sync::Arc};

use axum::Router;
use rmcp::{
    model::ProtocolVersion,
    transport::streamable_http_server::{
        session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServerHandler,
};

/// The only protocol revision TimeLex speaks.
pub const PROTOCOL_VERSION: &str = "2026-07-28";

/// The versions advertised by `server/discover` and accepted on every request.
pub const SUPPORTED_VERSIONS: &[ProtocolVersion] = &[ProtocolVersion::V_2026_07_28];

/// The supported versions, in the form [`ServerHandler::supported_protocol_versions`] wants.
///
/// Every handler served through [`router`] must override `supported_protocol_versions` with
/// this, or `rmcp` will fall back to accepting every version it knows.
pub fn supported_versions() -> Cow<'static, [ProtocolVersion]> {
    Cow::Borrowed(SUPPORTED_VERSIONS)
}

/// The transport settings TimeLex requires.
///
/// Construction validates the invariants that must never be silently skipped: an explicit
/// Origin allow-list (the spec's DNS-rebinding defence), an explicit Host allow-list that
/// includes the hostnames the service is actually deployed behind, and a non-zero body limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    allowed_origins: Vec<String>,
    allowed_hosts: Vec<String>,
    max_request_body_bytes: usize,
}

impl Config {
    /// 1 MiB. Enough for a `tools/call` on the Phase 1 tools, small enough that the gateway
    /// never buffers an unbounded request in memory.
    pub const DEFAULT_MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;

    /// `allowed_origins` are full origins (`scheme://host[:port]`); `allowed_hosts` are the
    /// `Host` header values the service answers on, with an optional port.
    pub fn new(allowed_origins: Vec<String>, allowed_hosts: Vec<String>) -> Result<Self, ConfigError> {
        if allowed_origins.is_empty() {
            return Err(ConfigError::NoAllowedOrigins);
        }
        if allowed_hosts.is_empty() {
            return Err(ConfigError::NoAllowedHosts);
        }
        for origin in &allowed_origins {
            let well_formed = origin
                .split_once("://")
                .is_some_and(|(scheme, authority)| !scheme.is_empty() && !authority.is_empty());
            if !well_formed {
                return Err(ConfigError::MalformedOrigin(origin.clone()));
            }
        }
        Ok(Self {
            allowed_origins,
            allowed_hosts,
            max_request_body_bytes: Self::DEFAULT_MAX_REQUEST_BODY_BYTES,
        })
    }

    pub fn with_max_request_body_bytes(mut self, bytes: usize) -> Result<Self, ConfigError> {
        if bytes == 0 {
            return Err(ConfigError::ZeroBodyLimit);
        }
        self.max_request_body_bytes = bytes;
        Ok(self)
    }

    /// The SDK configuration, with every unsafe default overridden.
    pub fn transport(&self) -> StreamableHttpServerConfig {
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_stateless_protocol_metadata_required(true)
            .with_allowed_origins(self.allowed_origins.iter().cloned())
            .with_allowed_hosts(self.allowed_hosts.iter().cloned())
            .with_max_request_body_bytes(self.max_request_body_bytes)
    }
}

/// Why a [`Config`] cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// An empty Origin list disables validation in `rmcp`, so it is never allowed.
    NoAllowedOrigins,
    /// An empty Host list makes every request fail with 403, so it is a configuration bug.
    NoAllowedHosts,
    /// An Origin without a scheme (`app.timelex.example` instead of `https://app.timelex.example`).
    MalformedOrigin(String),
    /// A zero body limit rejects every request.
    ZeroBodyLimit,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAllowedOrigins => write!(f, "at least one allowed origin is required"),
            Self::NoAllowedHosts => write!(f, "at least one allowed host is required"),
            Self::MalformedOrigin(origin) => {
                write!(f, "origin `{origin}` must be scheme://host[:port]")
            }
            Self::ZeroBodyLimit => write!(f, "the request body limit must be greater than zero"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A stateless Streamable HTTP service mounted at `/mcp`.
///
/// Each request gets a fresh handler and no session state is kept in process; durable state
/// (task rows, audit, outbox) belongs in Postgres behind the handler.
pub fn router<H>(config: &Config, handler: H) -> Router
where
    H: ServerHandler + Clone + 'static,
{
    let service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(NeverSessionManager::default()),
        config.transport(),
    );
    Router::new().nest_service("/mcp", service)
}
