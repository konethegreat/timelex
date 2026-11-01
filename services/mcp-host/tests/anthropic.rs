// Written by Kone Tshivhinda & Claude
//! The Anthropic adapter against a local stand-in: the key's route, the request shape, the reply
//! shape and the four error classes the loop acts on. No test reaches the real provider.

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use timelex_mcp_host::{
    AnthropicProvider, CompletionRequest, ModelCatalog, ModelError, ModelProvider, ModelTier, StopReason,
    Usage,
};
use timelex_vault::SecretValue;
use uuid::Uuid;

#[derive(Default)]
struct Recorded {
    headers: HeaderMap,
    body: String,
}

/// A local stand-in for `/v1/messages` that records what it saw and answers with one status.
async fn stub(status: StatusCode, body: &'static str) -> (String, Arc<Mutex<Recorded>>) {
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let seen = Arc::clone(&recorded);
    let app = Router::new().route(
        "/v1/messages",
        post(move |headers: HeaderMap, request_body: String| {
            let seen = Arc::clone(&seen);
            async move {
                let mut r = seen.lock().unwrap();
                r.headers = headers;
                r.body = request_body;
                (status, body)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), recorded)
}

/// A synthetic key, built at run time so no credential-shaped literal sits in this file.
fn key() -> SecretValue {
    SecretValue::parse(&format!("sk-ant-api03-{}", Uuid::new_v4().simple())).unwrap()
}

fn request() -> CompletionRequest {
    CompletionRequest {
        model: ModelTier::Haiku,
        system: "You attribute time.".into(),
        prompt: "Attribute this block.".into(),
        max_tokens: 1_024,
    }
}

const SUCCESS: &str = r#"{
    "content": [
        {"type": "text", "text": "Hello. "},
        {"type": "text", "text": "World."}
    ],
    "stop_reason": "end_turn",
    "usage": {
        "input_tokens": 12,
        "output_tokens": 3,
        "cache_read_input_tokens": 4,
        "cache_creation_input_tokens": 5
    }
}"#;

#[tokio::test]
async fn the_key_travels_in_a_header_and_the_tier_becomes_a_model_id() {
    let (base, recorded) = stub(StatusCode::OK, SUCCESS).await;
    let value = format!("sk-ant-api03-{}", Uuid::new_v4().simple());
    let provider = AnthropicProvider::new(&base, SecretValue::parse(&value).unwrap()).unwrap();

    provider.complete(request()).await.unwrap();

    let recorded = recorded.lock().unwrap();
    assert_eq!(recorded.headers["anthropic-version"], "2023-06-01");
    assert_eq!(recorded.headers["content-type"], "application/json");
    // The value the provider was built with is the value it sends.
    assert_eq!(recorded.headers["x-api-key"], value);
    assert!(!recorded.body.contains(&value), "the key never enters the body");

    let sent: serde_json::Value = serde_json::from_str(&recorded.body).unwrap();
    assert_eq!(sent["model"], "claude-haiku-5-5");
    assert_eq!(sent["max_tokens"], 1_024);
    assert_eq!(sent["system"], "You attribute time.");
    assert_eq!(sent["messages"][0]["role"], "user");
    assert_eq!(sent["messages"][0]["content"], "Attribute this block.");
}

#[tokio::test]
async fn a_success_is_text_plus_usage() {
    let (base, _) = stub(StatusCode::OK, SUCCESS).await;
    let provider = AnthropicProvider::new(&base, key()).unwrap();

    let completion = provider.complete(request()).await.unwrap();

    assert_eq!(completion.text, "Hello. World.");
    assert_eq!(completion.stop_reason, StopReason::EndTurn);
    assert_eq!(
        completion.usage,
        Usage {
            input_tokens: 12,
            output_tokens: 3,
            cache_read_tokens: 4,
            cache_write_tokens: 5,
        }
    );
}

#[tokio::test]
async fn a_model_this_key_cannot_use_is_not_a_key_failure() {
    // 404 and 403 are the two shapes a workspace without model access returns; both must let the
    // loop try the next rung rather than stop the run.
    for status in [StatusCode::NOT_FOUND, StatusCode::FORBIDDEN] {
        let (base, _) = stub(status, r#"{"type":"error","error":{"type":"permission_error"}}"#).await;
        let provider = AnthropicProvider::new(&base, key()).unwrap();
        assert_eq!(
            provider.complete(request()).await,
            Err(ModelError::ModelUnavailable),
            "{status}"
        );
    }
}

#[tokio::test]
async fn a_rejected_key_is_a_key_failure() {
    let (base, _) = stub(
        StatusCode::UNAUTHORIZED,
        r#"{"type":"error","error":{"type":"authentication_error","message":"the-provider-message"}}"#,
    )
    .await;
    let provider = AnthropicProvider::new(&base, key()).unwrap();

    let error = provider.complete(request()).await.unwrap_err();

    assert_eq!(error, ModelError::KeyRejected);
    // The provider's words never travel in an error, a log or a body.
    assert!(!format!("{error:?}").contains("the-provider-message"));
    assert!(!error.to_string().contains("the-provider-message"));
}

#[tokio::test]
async fn rate_limits_and_server_errors_are_unreachable() {
    for status in [
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let (base, _) = stub(status, "{}").await;
        let provider = AnthropicProvider::new(&base, key()).unwrap();
        assert_eq!(
            provider.complete(request()).await,
            Err(ModelError::Unreachable),
            "{status}"
        );
    }
}

#[tokio::test]
async fn a_reply_without_text_is_unusable() {
    let (base, _) = stub(
        StatusCode::OK,
        r#"{"content":[{"type":"thinking","thinking":"..."}],"usage":{"input_tokens":1,"output_tokens":0}}"#,
    )
    .await;
    let provider = AnthropicProvider::new(&base, key()).unwrap();

    assert_eq!(provider.complete(request()).await, Err(ModelError::Unusable));
}

#[tokio::test]
async fn a_malformed_success_is_unusable() {
    let (base, _) = stub(StatusCode::OK, "not json").await;
    let provider = AnthropicProvider::new(&base, key()).unwrap();

    assert_eq!(provider.complete(request()).await, Err(ModelError::Unusable));
}

#[tokio::test]
async fn a_provider_that_is_not_there_is_unreachable() {
    // Port 9 (discard) on loopback refuses connections in every environment we test on.
    let provider = AnthropicProvider::new("http://127.0.0.1:9", key()).unwrap();

    assert_eq!(provider.complete(request()).await, Err(ModelError::Unreachable));
}

#[tokio::test]
async fn the_catalog_can_be_replaced() {
    let (base, recorded) = stub(StatusCode::OK, SUCCESS).await;
    let catalog = ModelCatalog::new(
        "claude-haiku-test",
        "claude-sonnet-test",
        "claude-opus-test",
        "claude-fable-test",
    );
    let provider = AnthropicProvider::new(&base, key())
        .unwrap()
        .with_catalog(catalog);

    provider.complete(request()).await.unwrap();

    let sent: serde_json::Value = serde_json::from_str(&recorded.lock().unwrap().body).unwrap();
    assert_eq!(sent["model"], "claude-haiku-test");
}
