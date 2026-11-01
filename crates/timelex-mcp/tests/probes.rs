// Written by Kone Tshivhinda & Claude
// Contract tests: a Rust port of docs/research/spikes/p1-08-rmcp/probe.sh against the
// timelex-mcp adapter, so an rmcp bump that changes transport behaviour fails CI.

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use rmcp::{model::*, service::RequestContext, ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};

use timelex_mcp::{Config, ConfigError};

const VERSION: &str = "2026-07-28";
const PROVIDER_ORIGIN: &str = "https://app.timelex.example";

fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": VERSION,
        "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

fn meta_with_tasks() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": VERSION,
        "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {
            "extensions": {"io.modelcontextprotocol/tasks": {}}
        }
    })
}

fn schema(value: Value) -> Arc<JsonObject> {
    Arc::new(value.as_object().cloned().expect("schema object"))
}

#[derive(Clone, Default)]
struct TestHandler {
    tasks: Arc<Mutex<HashMap<String, Instant>>>,
}

impl ServerHandler for TestHandler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
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
            schema(json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            })),
        );
        let slow = Tool::new(
            "slow_job",
            "Pretend to backfill a mailbox; returns a task",
            schema(json!({"type": "object", "properties": {}})),
        );
        Ok(ListToolsResult::with_all_items(vec![echo, slow]))
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
            "slow_job" => {
                static NEXT: AtomicU64 = AtomicU64::new(1);
                let id = format!("task-{}", NEXT.fetch_add(1, Ordering::Relaxed));
                self.tasks.lock().unwrap().insert(id.clone(), Instant::now());
                let task = Task::new(id, TaskStatus::Working, timestamp(), timestamp())
                    .with_ttl_ms(60_000)
                    .with_poll_interval_ms(500);
                Ok(CallToolResponse::Task(CreateTaskResult::new(task)))
            }
            other => Err(McpError::invalid_params(format!("unknown tool {other}"), None)),
        }
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        let started = self
            .tasks
            .lock()
            .unwrap()
            .get(&request.task_id)
            .copied()
            .ok_or_else(|| McpError::invalid_params("unknown task", None))?;
        let base = Task::new(
            request.task_id.clone(),
            TaskStatus::Working,
            timestamp(),
            timestamp(),
        );
        let payload = if started.elapsed() > Duration::from_millis(100) {
            let result = CallToolResult::success(vec![ContentBlock::text("backfill done")]);
            TaskPayload::Completed {
                result: serde_json::to_value(result)
                    .unwrap()
                    .as_object()
                    .cloned()
                    .unwrap(),
            }
        } else {
            TaskPayload::Working
        };
        Ok(GetTaskResult::new(DetailedTask::new(base, payload)))
    }
}

fn timestamp() -> String {
    "2026-11-01T00:00:00Z".to_string()
}

struct TestServer {
    url: String,
    client: reqwest::Client,
    server: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn start() -> Self {
        let config = Config::new(vec![PROVIDER_ORIGIN.to_string()], vec!["127.0.0.1".to_string()])
            .expect("valid config");
        let app = timelex_mcp::router(&config, TestHandler::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            client: reqwest::Client::new(),
            server,
        }
    }

    async fn post(&self, headers: &[(&str, &str)], body: Value) -> Reply {
        let mut request = self
            .client
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.json(&body).send().await.unwrap();
        Reply::read(response).await
    }

    async fn get(&self) -> Reply {
        Reply::read(
            self.client
                .get(&self.url)
                .header("accept", "text/event-stream")
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    async fn delete(&self) -> Reply {
        Reply::read(self.client.delete(&self.url).send().await.unwrap()).await
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.server.abort();
    }
}

struct Reply {
    status: u16,
    headers: reqwest::header::HeaderMap,
    body: String,
}

impl Reply {
    async fn read(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        Self {
            status,
            headers,
            body: response.text().await.unwrap(),
        }
    }

    fn json(&self) -> Value {
        let body = if self.body.trim_start().starts_with("data:") {
            self.body
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .expect("SSE data line")
        } else {
            self.body.as_str()
        };
        serde_json::from_str(body).expect("json-rpc body")
    }

    fn error_code(&self) -> i64 {
        self.json()["error"]["code"].as_i64().expect("error code")
    }

    fn result(&self) -> Value {
        self.json()["result"].clone()
    }
}

fn headers(method: &str) -> Vec<(&'static str, &str)> {
    vec![("MCP-Protocol-Version", VERSION), ("Mcp-Method", method)]
}

const SERVER_NAME: &str = "timelex-test";

#[tokio::test]
async fn probe_01_discover_advertises_only_the_modern_protocol() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &headers("server/discover"),
            json!({"jsonrpc": "2.0", "id": 1, "method": "server/discover",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 200, "{}", reply.body);
    let result = reply.result();
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"], json!([VERSION]));
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        SERVER_NAME
    );
}

#[tokio::test]
async fn probe_02_tools_list_serves_the_contract_tools() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &headers("tools/list"),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 200, "{}", reply.body);
    let result = reply.result();
    let names: Vec<&str> = result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["echo", "slow_job"]);
    assert_eq!(result["resultType"], "complete");
}

#[tokio::test]
async fn probe_03_tools_call_echoes_over_stateless_http() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/call"),
                ("Mcp-Name", "echo"),
            ],
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                   "params": {"name": "echo", "arguments": {"text": "hello"}, "_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 200, "{}", reply.body);
    let result = reply.result();
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["content"][0]["text"], "hello");
    assert_eq!(result["isError"], false);
}

#[tokio::test]
async fn probe_04_tools_call_returns_a_task_when_the_client_asks() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/call"),
                ("Mcp-Name", "slow_job"),
            ],
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                   "params": {"name": "slow_job", "arguments": {}, "_meta": meta_with_tasks()}}),
        )
        .await;

    assert_eq!(reply.status, 200, "{}", reply.body);
    let result = reply.result();
    assert_eq!(result["resultType"], "task");
    assert!(result["taskId"].as_str().unwrap().starts_with("task-"));
    assert_eq!(result["status"], "working");
    assert_eq!(result["ttlMs"], 60_000);
    assert_eq!(result["pollIntervalMs"], 500);
}

#[tokio::test]
async fn probe_05_tools_call_without_the_tasks_capability_is_rejected() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/call"),
                ("Mcp-Name", "slow_job"),
            ],
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
                   "params": {"name": "slow_job", "arguments": {}, "_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32021);
    assert_eq!(
        reply.json()["error"]["data"]["requiredCapabilities"]["extensions"]["io.modelcontextprotocol/tasks"],
        json!({})
    );
}

#[tokio::test]
async fn probe_06_task_state_outlives_the_request_that_created_it() {
    let server = TestServer::start().await;
    let created = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/call"),
                ("Mcp-Name", "slow_job"),
            ],
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
                   "params": {"name": "slow_job", "arguments": {}, "_meta": meta_with_tasks()}}),
        )
        .await;
    assert_eq!(created.status, 200, "{}", created.body);
    let task_id = created.result()["taskId"].as_str().unwrap().to_string();

    let task_headers = [
        ("MCP-Protocol-Version", VERSION),
        ("Mcp-Method", "tasks/get"),
        ("Mcp-Name", task_id.as_str()),
    ];
    let body = json!({"jsonrpc": "2.0", "id": 7, "method": "tasks/get",
                      "params": {"taskId": task_id, "_meta": meta_with_tasks()}});

    let immediate = server.post(&task_headers, body.clone()).await;
    assert_eq!(immediate.status, 200, "{}", immediate.body);
    assert_eq!(immediate.result()["status"], "working");

    tokio::time::sleep(Duration::from_millis(200)).await;
    let later = server.post(&task_headers, body).await;
    assert_eq!(later.status, 200, "{}", later.body);
    let result = later.result();
    assert_eq!(result["status"], "completed");
    assert_eq!(result["result"]["content"][0]["text"], "backfill done");
}

#[tokio::test]
async fn probe_07_missing_request_meta_is_invalid_params() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &headers("tools/list"),
            json!({"jsonrpc": "2.0", "id": 8, "method": "tools/list", "params": {}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32602);
}

#[tokio::test]
async fn probe_08_mcp_name_header_must_match_the_body() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/call"),
                ("Mcp-Name", "slow_job"),
            ],
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
                   "params": {"name": "echo", "arguments": {"text": "x"}, "_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32020);
}

#[tokio::test]
async fn probe_09_mcp_method_header_must_match_the_body() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &headers("tools/list"),
            json!({"jsonrpc": "2.0", "id": 10, "method": "tools/call",
                   "params": {"name": "echo", "arguments": {"text": "x"}, "_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32020);
}

#[tokio::test]
async fn probe_10_missing_mcp_method_header_is_rejected() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[("MCP-Protocol-Version", VERSION)],
            json!({"jsonrpc": "2.0", "id": 11, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32020);
}

#[tokio::test]
async fn probe_11_unknown_protocol_version_lists_what_is_supported() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", "1900-01-01"),
                ("Mcp-Method", "tools/list"),
            ],
            json!({"jsonrpc": "2.0", "id": 12, "method": "tools/list",
                   "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01",
                                        "io.modelcontextprotocol/clientCapabilities": {}}}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32022);
    assert_eq!(reply.json()["error"]["data"]["supported"], json!([VERSION]));
}

#[tokio::test]
async fn probe_12_header_and_body_versions_must_agree() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &headers("tools/list"),
            json!({"jsonrpc": "2.0", "id": 13, "method": "tools/list",
                   "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2025-11-25",
                                        "io.modelcontextprotocol/clientCapabilities": {}}}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32020);
}

#[tokio::test]
async fn probe_13_get_is_not_allowed() {
    let server = TestServer::start().await;
    let reply = server.get().await;

    assert_eq!(reply.status, 405, "{}", reply.body);
    assert_eq!(reply.headers["allow"], "POST");
}

#[tokio::test]
async fn probe_14_delete_is_not_allowed() {
    let server = TestServer::start().await;
    let reply = server.delete().await;

    assert_eq!(reply.status, 405, "{}", reply.body);
    assert_eq!(reply.headers["allow"], "POST");
}

#[tokio::test]
async fn probe_15_session_id_is_ignored_and_never_issued() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/list"),
                ("Mcp-Session-Id", "abc"),
            ],
            json!({"jsonrpc": "2.0", "id": 14, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 200, "{}", reply.body);
    assert!(!reply.headers.contains_key("mcp-session-id"));
}

#[tokio::test]
async fn probe_16_disallowed_origin_is_forbidden() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("Origin", "https://evil.example"),
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/list"),
            ],
            json!({"jsonrpc": "2.0", "id": 15, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 403, "{}", reply.body);
}

#[tokio::test]
async fn probe_17_unknown_host_is_forbidden() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[
                ("Host", "timelex.example"),
                ("MCP-Protocol-Version", VERSION),
                ("Mcp-Method", "tools/list"),
            ],
            json!({"jsonrpc": "2.0", "id": 16, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 403, "{}", reply.body);
}

#[tokio::test]
async fn probe_18_legacy_initialize_is_rejected() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[],
            json!({"jsonrpc": "2.0", "id": 17, "method": "initialize",
                   "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                              "clientInfo": {"name": "legacy", "version": "0"}}}),
        )
        .await;

    // The legacy path delivers its JSON-RPC rejection over the SSE stream, so the status is 200
    // while the request itself is refused. What must never change is that no version is agreed.
    assert!(
        matches!(reply.status, 200 | 400),
        "unexpected status {}: {}",
        reply.status,
        reply.body
    );
    assert_eq!(reply.error_code(), -32022);
    assert!(reply.json()["result"].is_null(), "{}", reply.body);
}

#[tokio::test]
async fn strict_metadata_requires_the_protocol_header() {
    let server = TestServer::start().await;
    let reply = server
        .post(
            &[],
            json!({"jsonrpc": "2.0", "id": 18, "method": "tools/list",
                   "params": {"_meta": meta()}}),
        )
        .await;

    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(reply.error_code(), -32020);
}

#[test]
fn config_requires_explicit_origins_and_hosts() {
    assert_eq!(
        Config::new(Vec::<String>::new(), vec!["127.0.0.1".to_string()]).err(),
        Some(ConfigError::NoAllowedOrigins)
    );
    assert_eq!(
        Config::new(vec![PROVIDER_ORIGIN.to_string()], Vec::<String>::new()).err(),
        Some(ConfigError::NoAllowedHosts)
    );
    assert_eq!(
        Config::new(
            vec!["app.timelex.example".to_string()],
            vec!["127.0.0.1".to_string()]
        )
        .err(),
        Some(ConfigError::MalformedOrigin("app.timelex.example".to_string()))
    );
    assert_eq!(
        Config::new(vec![PROVIDER_ORIGIN.to_string()], vec!["127.0.0.1".to_string()])
            .unwrap()
            .with_max_request_body_bytes(0)
            .err(),
        Some(ConfigError::ZeroBodyLimit)
    );
}

#[test]
fn config_transport_is_stateless_and_strict() {
    let config = Config::new(vec![PROVIDER_ORIGIN.to_string()], vec!["127.0.0.1".to_string()])
        .unwrap()
        .with_max_request_body_bytes(1_048_576)
        .unwrap();
    let transport = config.transport();

    assert!(!transport.legacy_session_mode);
    assert!(transport.stateless_protocol_metadata_required);
    assert_eq!(transport.allowed_origins, [PROVIDER_ORIGIN]);
    assert_eq!(transport.allowed_hosts, ["127.0.0.1"]);
    assert_eq!(transport.max_request_body_bytes, 1_048_576);
}

#[test]
fn supported_versions_are_modern_only() {
    let versions = timelex_mcp::supported_versions();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0], ProtocolVersion::V_2026_07_28);
}

#[test]
fn protocol_version_constant_is_the_modern_revision() {
    assert_eq!(timelex_mcp::PROTOCOL_VERSION, VERSION);
}
