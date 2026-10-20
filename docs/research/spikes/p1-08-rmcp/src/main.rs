// Written by Kone Tshivhinda & Claude
// P1-08 spike: does rmcp 3.6.0 serve MCP 2026-07-28 statelessly over Streamable HTTP,
// including tools/list, tools/call and the Tasks extension? Scratch code, not product code.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
    },
};
use serde_json::json;

#[derive(Clone, Default)]
struct Spike {
    // task_id -> when it was created. A real service keeps this in Postgres (stateless servers).
    tasks: Arc<Mutex<HashMap<String, Instant>>>,
}

fn schema(v: serde_json::Value) -> Arc<JsonObject> {
    Arc::new(v.as_object().cloned().unwrap())
}

impl ServerHandler for Spike {
    fn get_info(&self) -> ServerConfig {
        let caps = ServerCapabilities::builder()
            .enable_tools()
            .enable_tasks()
            .build();
        ServerConfig::new(caps)
            .with_server_info(Implementation::new("timelex-spike", "0.0.1"))
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
            "Pretend to back-fill a mailbox; returns a task",
            schema(json!({ "type": "object", "properties": {} })),
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
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                Ok(CallToolResult::success(vec![ContentBlock::text(text.to_string())]).into())
            }
            "slow_job" => {
                let id = format!("task-{}", chrono::Utc::now().timestamp_millis());
                self.tasks.lock().unwrap().insert(id.clone(), Instant::now());
                let now = chrono::Utc::now().to_rfc3339();
                let task = Task::new(id, TaskStatus::Working, now.clone(), now)
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
        let started = self.tasks.lock().unwrap().get(&request.task_id).copied();
        let Some(started) = started else {
            return Err(McpError::invalid_params("unknown task", None));
        };
        let now = chrono::Utc::now().to_rfc3339();
        let base = Task::new(request.task_id.clone(), TaskStatus::Working, now.clone(), now);
        let payload = if started.elapsed() > Duration::from_secs(2) {
            let result = CallToolResult::success(vec![ContentBlock::text("backfill done")]);
            TaskPayload::Completed {
                result: serde_json::to_value(result).unwrap().as_object().cloned().unwrap(),
            }
        } else {
            TaskPayload::Working
        };
        Ok(GetTaskResult::new(DetailedTask::new(base, payload)))
    }
}

#[tokio::main]
async fn main() {
    let strict = std::env::args().any(|a| a == "--strict");
    let mut config = StreamableHttpServerConfig::default().with_legacy_session_mode(false);
    if strict {
        config = config.with_stateless_protocol_metadata_required(true);
    }
    let spike = Spike::default();
    let service = StreamableHttpService::new(
        move || Ok(spike.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let port = if strict { 8091 } else { 8090 };
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    println!("listening on 127.0.0.1:{port} (strict={strict})");
    axum::serve(listener, app).await.unwrap();
}

