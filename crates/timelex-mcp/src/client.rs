// Written by Kone Tshivhinda & Claude
//! The client half of the adapter: how TimeLex connects to an MCP server.
//!
//! The host is the inward MCP client (ADR-0003): it calls TimeLex's own services and, later,
//! third-party servers. The rules are deliberately narrow: HTTPS everywhere except a loopback
//! address, the pinned 2026-07-28 discovery lifecycle only (no `initialize`, no sessions), and a
//! bearer token if one is supplied. A server that only speaks an older revision is refused
//! rather than negotiated down, and a protocol error is surfaced without the server's text.

use std::fmt;

use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ClientCapabilities,
        ClientConfig as RmcpClientConfig, Implementation, JsonObject, ListToolsResult, ProtocolVersion,
    },
    service::{ClientInitializeError, RunningService, ServiceError},
    transport::{streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport},
    ClientLifecycleMode, ClientServiceExt, RoleClient,
};
use url::{Host, Url};
use zeroize::Zeroizing;

use crate::SUPPORTED_VERSIONS;

/// The only protocol revision the client offers.
const PINNED_VERSION: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// The longest bearer token the client accepts.
const MAX_BEARER_LEN: usize = 8192;

/// A validated MCP server address and the credential, if one, used to reach it.
///
/// Construction is the only way in, so an invalid address cannot reach the transport.
#[derive(Clone, PartialEq, Eq)]
pub struct ClientTarget {
    endpoint: Url,
    bearer: Option<Zeroizing<String>>,
}

impl ClientTarget {
    /// Validate `endpoint`. HTTPS is required; plain HTTP is allowed only for loopback addresses,
    /// where the traffic never leaves the machine.
    pub fn new(endpoint: &str) -> Result<Self, TargetError> {
        let url = Url::parse(endpoint).map_err(|_| TargetError::NotAUrl)?;
        match url.scheme() {
            "https" => {}
            "http" if is_loopback(&url) => {}
            "http" => return Err(TargetError::PlainHttpOutsideLoopback),
            _ => return Err(TargetError::UnsupportedScheme),
        }
        if url.host().is_none() {
            return Err(TargetError::NotAUrl);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(TargetError::CredentialsInUrl);
        }
        if url.fragment().is_some() {
            return Err(TargetError::FragmentInUrl);
        }
        Ok(Self {
            endpoint: url,
            bearer: None,
        })
    }

    /// Send `token` as `Authorization: Bearer {token}` on every request. The token is kept out of
    /// [`Debug`] output and wiped when the target is dropped.
    pub fn with_bearer_token(mut self, token: &str) -> Result<Self, TargetError> {
        if token.is_empty() || token.len() > MAX_BEARER_LEN || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(TargetError::InvalidBearerToken);
        }
        self.bearer = Some(Zeroizing::new(token.to_owned()));
        Ok(self)
    }

    fn transport(&self) -> StreamableHttpClientTransportConfig {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.endpoint.as_str())
            // TimeLex servers are stateless and never issue a session. A 404 is a real routing
            // error, not an expired session, and re-running discovery would hide it.
            .reinit_on_expired_session(false);
        config.allow_stateless = true;
        if let Some(token) = &self.bearer {
            config = config.auth_header(token.as_str());
        }
        config
    }
}

impl fmt::Debug for ClientTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientTarget")
            .field("endpoint", &self.endpoint.as_str())
            .field("bearer", &self.bearer.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// Why a target cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    /// The string is not an absolute URL.
    NotAUrl,
    /// Only `http` and `https` are MCP transports.
    UnsupportedScheme,
    /// Plain HTTP would send the bearer token in clear text.
    PlainHttpOutsideLoopback,
    /// A URL must not carry a username or password; credentials belong in the header.
    CredentialsInUrl,
    /// A fragment is not sent to the server and only hints at a client-side mistake.
    FragmentInUrl,
    /// Empty, oversized, or containing whitespace or control bytes.
    InvalidBearerToken,
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAUrl => write!(f, "the MCP endpoint must be an absolute URL"),
            Self::UnsupportedScheme => write!(f, "the MCP endpoint must use http or https"),
            Self::PlainHttpOutsideLoopback => {
                write!(f, "plain HTTP is only allowed for a loopback MCP endpoint")
            }
            Self::CredentialsInUrl => {
                write!(f, "the MCP endpoint must not contain a username or password")
            }
            Self::FragmentInUrl => write!(f, "the MCP endpoint must not contain a fragment"),
            Self::InvalidBearerToken => write!(f, "the bearer token is empty or not printable"),
        }
    }
}

impl std::error::Error for TargetError {}

/// Why a client could not start or a request did not produce a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    /// Discovery was refused, unreachable, or answered with something unexpected.
    Startup,
    /// The server does not offer the pinned protocol revision.
    Protocol,
    /// The request failed in the transport.
    Transport,
    /// The server answered with a JSON-RPC error. Only the code is kept; the server's message
    /// never reaches a log or an error.
    Rpc { code: i32 },
    /// The server asked for client input, which the host does not provide.
    InputRequired,
    /// The server returned a task, which the host does not poll.
    Task,
    /// The connection was closed or cancelled.
    Closed,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Startup => write!(f, "the MCP server did not complete the discovery handshake"),
            Self::Protocol => write!(f, "the MCP server does not speak {}", PINNED_VERSION),
            Self::Transport => write!(f, "the MCP request did not complete"),
            Self::Rpc { code } => write!(f, "the MCP server returned protocol error {code}"),
            Self::InputRequired => {
                write!(f, "the MCP server asked for client input the host cannot give")
            }
            Self::Task => write!(f, "the MCP server returned a task the host does not poll"),
            Self::Closed => write!(f, "the MCP connection closed"),
        }
    }
}

impl std::error::Error for ClientError {}

/// A connected MCP server.
///
/// The connection is created with discovery and survives until [`close`](Self::close) or drop.
pub struct McpClient {
    service: RunningService<RoleClient, RmcpClientConfig>,
}

impl McpClient {
    /// Discover the server and agree on the pinned protocol revision. There is no fallback: a
    /// server that only knows an earlier revision is an error, not a downgrade.
    pub async fn connect(target: &ClientTarget) -> Result<Self, ClientError> {
        let config = RmcpClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("timelex-host", env!("CARGO_PKG_VERSION")),
        )
        .with_protocol_version(PINNED_VERSION);
        let service = config
            .serve_with_lifecycle(
                StreamableHttpClientTransport::from_config(target.transport()),
                ClientLifecycleMode::Discover {
                    preferred_versions: SUPPORTED_VERSIONS.to_vec(),
                },
            )
            .await
            .map_err(|error| match error {
                ClientInitializeError::NoCompatibleProtocolVersion { .. } => ClientError::Protocol,
                _ => ClientError::Startup,
            })?;
        Ok(Self { service })
    }

    /// The tools the server offers, one page.
    pub async fn list_tools(&self) -> Result<ListToolsResult, ClientError> {
        self.service.list_tools(None).await.map_err(map_service_error)
    }

    /// Call one tool once. A server that wants to pause for client input, or that hands back a
    /// task, is refused: the host only understands a finished result.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, ClientError> {
        let mut request = CallToolRequestParams::new(name.to_owned());
        if let Some(arguments) = arguments {
            request = request.with_arguments(arguments);
        }
        match self
            .service
            .call_tool_once(request)
            .await
            .map_err(map_service_error)?
        {
            CallToolResponse::Complete(result) => Ok(result),
            CallToolResponse::InputRequired(_) => Err(ClientError::InputRequired),
            CallToolResponse::Task(_) => Err(ClientError::Task),
            _ => Err(ClientError::Protocol),
        }
    }

    /// Close the connection and stop its background task.
    pub async fn close(self) -> Result<(), ClientError> {
        self.service
            .cancel()
            .await
            .map(|_| ())
            .map_err(|_| ClientError::Closed)
    }
}

impl fmt::Debug for McpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpClient").finish_non_exhaustive()
    }
}

fn map_service_error(error: ServiceError) -> ClientError {
    match error {
        ServiceError::McpError(error) => ClientError::Rpc { code: error.code.0 },
        ServiceError::TransportClosed | ServiceError::Cancelled { .. } => ClientError::Closed,
        ServiceError::UnexpectedResponse => ClientError::Protocol,
        _ => ClientError::Transport,
    }
}
