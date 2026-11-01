// Written by Kone Tshivhinda & Claude
// Contract tests for the MCP client: it must speak only the pinned 2026-07-28 discovery
// lifecycle over a stateless transport, and must never carry the bearer token anywhere except
// the Authorization header.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::post,
    Json, Router,
};
use rmcp::{model::*, service::RequestContext, ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};

use timelex_mcp::{
    client::{ClientError, ClientTarget, McpClient, TargetError},
    Config,
};

const VERSION: &str = "2026-07-28";
const PROVIDER_ORIGIN: &str = "https://app.timelex.example";

#[derive(Clone, Default)]
struct Echo;

impl ServerHandler for Echo {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("timelex-test", "0.0.0"))
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        timelex_mcp::supported_versions()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let echo = Tool::new(
            "echo",
            "Echo the given text back (read-only)",
            Arc::new(
                json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                })
                .as_object()
                .cloned()
                .expect("schema object"),
            ),
        );
        Ok(ListToolsResult::with_all_items(vec![echo]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        match request.name.as_ref() {
            "echo" => {
                let text = request
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                Ok(CallToolResult::success(vec![ContentBlock::text(text.to_string())]).into())
            }
            other => Err(McpError::invalid_params(format!("unknown tool {other}"), None)),
        }
    }
}

#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<axum::http::HeaderMap>>>);

impl Seen {
    fn headers(&self) -> Vec<axum::http::HeaderMap> {
        self.0.lock().unwrap().clone()
    }
}

async fn record(State(seen): State<Seen>, request: Request, next: Next) -> Response {
    seen.0.lock().unwrap().push(request.headers().clone());
    next.run(request).await
}

struct TestServer {
    url: String,
    seen: Seen,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn start() -> Self {
        let config = Config::new(vec![PROVIDER_ORIGIN.to_string()], vec!["127.0.0.1".to_string()])
            .expect("valid config");
        let seen = Seen::default();
        let app =
            timelex_mcp::router(&config, Echo).layer(middleware::from_fn_with_state(seen.clone(), record));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            seen,
            task,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A stub that answers every JSON-RPC request with the same canned reply, recording the methods
/// it was asked for.
type Reply = Arc<dyn Fn(&str, Value) -> Value + Send + Sync>;

#[derive(Clone)]
struct StubState {
    methods: Arc<Mutex<Vec<String>>>,
    reply: Reply,
}

async fn stub_handle(State(state): State<StubState>, body: String) -> Json<Value> {
    let request: Value = serde_json::from_str(&body).expect("json-rpc body");
    let method = request["method"].as_str().unwrap_or("").to_string();
    state.methods.lock().unwrap().push(method.clone());
    Json((state.reply)(&method, request))
}

struct StubServer {
    url: String,
    methods: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl StubServer {
    async fn reply_with(reply: Reply) -> Self {
        let methods = Arc::new(Mutex::new(Vec::new()));
        let state = StubState {
            methods: methods.clone(),
            reply,
        };
        let app = Router::new().route("/mcp", post(stub_handle)).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            methods,
            task,
        }
    }

    fn methods(&self) -> Vec<String> {
        self.methods.lock().unwrap().clone()
    }
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn error_for(request: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": request["id"].clone(),
           "error": {"code": code, "message": message}})
}

#[tokio::test]
async fn client_lists_the_server_tools() {
    let server = TestServer::start().await;
    let client = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect("connect");

    let tools = client.list_tools().await.expect("list tools");
    let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names, ["echo"]);

    client.close().await.expect("close");
}

#[tokio::test]
async fn client_calls_a_tool_and_returns_its_result() {
    let server = TestServer::start().await;
    let client = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect("connect");

    let mut arguments = serde_json::Map::new();
    arguments.insert("text".into(), json!("hello"));
    let result = client
        .call_tool("echo", Some(arguments))
        .await
        .expect("call tool");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content[0].as_text().unwrap().text, "hello");

    client.close().await.expect("close");
}

#[tokio::test]
async fn client_uses_discovery_only_and_never_a_session() {
    let server = TestServer::start().await;
    let client = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect("connect");
    client.list_tools().await.expect("list tools");
    client.close().await.expect("close");

    let seen = server.seen.headers();
    assert!(!seen.is_empty(), "the server should have been called");
    for headers in &seen {
        assert!(
            !headers.contains_key("mcp-session-id"),
            "the client sent a session id"
        );
        assert_eq!(
            headers.get("mcp-protocol-version").unwrap().to_str().unwrap(),
            VERSION
        );
    }
}

#[tokio::test]
async fn client_sends_the_bearer_token_only_in_the_authorization_header() {
    let server = TestServer::start().await;
    let token = format!("test-bearer-{}", uuid::Uuid::new_v4().simple());
    let target = ClientTarget::new(&server.url)
        .unwrap()
        .with_bearer_token(&token)
        .expect("valid token");
    let client = McpClient::connect(&target).await.expect("connect");
    client.list_tools().await.expect("list tools");
    client.close().await.expect("close");

    let seen = server.seen.headers();
    assert!(!seen.is_empty());
    for headers in &seen {
        assert_eq!(
            headers.get("authorization").unwrap().to_str().unwrap(),
            format!("Bearer {token}")
        );
        for (name, value) in headers {
            if name != "authorization" {
                assert!(
                    !value.to_str().unwrap_or("").contains(&token),
                    "the token appeared in header {name}"
                );
            }
        }
    }
    assert!(
        !format!("{target:?}").contains(&token),
        "the target leaked its token"
    );
}

#[tokio::test]
async fn client_refuses_a_legacy_only_server_without_initializing() {
    let server = StubServer::reply_with(Arc::new(|_method, request| {
        error_for(&request, -32601, "Method not found")
    }))
    .await;
    let error = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect_err("a legacy-only server must be refused");

    assert!(matches!(error, ClientError::Startup), "{error:?}");
    assert_eq!(server.methods(), ["server/discover"]);
}

#[tokio::test]
async fn client_refuses_a_server_that_does_not_speak_the_pinned_version() {
    let server = StubServer::reply_with(Arc::new(|method, request| {
        if method == "server/discover" {
            let result =
                DiscoverResult::new(vec![ProtocolVersion::V_2025_11_25], ServerCapabilities::default());
            json!({"jsonrpc": "2.0", "id": request["id"].clone(),
                   "result": serde_json::to_value(result).unwrap()})
        } else {
            error_for(&request, -32601, "Method not found")
        }
    }))
    .await;

    let error = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect_err("an older-only server must be refused");
    assert!(matches!(error, ClientError::Protocol), "{error:?}");
}

#[tokio::test]
async fn client_surfaces_a_protocol_error_without_the_server_message() {
    let server = TestServer::start().await;
    let client = McpClient::connect(&ClientTarget::new(&server.url).unwrap())
        .await
        .expect("connect");

    let error = client
        .call_tool("no_such_tool", None)
        .await
        .expect_err("an unknown tool is a protocol error");
    assert_eq!(error, ClientError::Rpc { code: -32602 });
    assert!(!format!("{error:?}").contains("no_such_tool"));

    client.close().await.expect("close");
}

#[test]
fn target_requires_tls_outside_loopback() {
    assert!(ClientTarget::new("https://mcp.timelex.example/mcp").is_ok());
    assert!(ClientTarget::new("http://127.0.0.1:8080/mcp").is_ok());
    assert!(ClientTarget::new("http://[::1]:8080/mcp").is_ok());
    assert!(ClientTarget::new("http://localhost:8080/mcp").is_ok());

    assert_eq!(
        ClientTarget::new("http://mcp.timelex.example/mcp").err(),
        Some(TargetError::PlainHttpOutsideLoopback)
    );
    assert_eq!(
        ClientTarget::new("ftp://mcp.timelex.example/mcp").err(),
        Some(TargetError::UnsupportedScheme)
    );
    assert_eq!(ClientTarget::new("not a url").err(), Some(TargetError::NotAUrl));
    assert_eq!(
        ClientTarget::new("https://user:pass@mcp.timelex.example/mcp").err(),
        Some(TargetError::CredentialsInUrl)
    );
    assert_eq!(
        ClientTarget::new("https://mcp.timelex.example/mcp#fragment").err(),
        Some(TargetError::FragmentInUrl)
    );
}

#[test]
fn target_rejects_an_unusable_bearer_token() {
    let target = ClientTarget::new("https://mcp.timelex.example/mcp").unwrap();
    assert_eq!(
        target.clone().with_bearer_token("").err(),
        Some(TargetError::InvalidBearerToken)
    );
    assert_eq!(
        target.with_bearer_token("has a space").err(),
        Some(TargetError::InvalidBearerToken)
    );
}
