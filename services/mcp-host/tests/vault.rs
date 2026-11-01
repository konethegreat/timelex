// Written by Kone Tshivhinda & Claude
//! The lease client against a stub vault on loopback. The stub verifies the workload token exactly
//! as the real vault does, so a test proves the client signs the right claims for the right path.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{header, HeaderMap, Method, StatusCode, Uri};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use time::OffsetDateTime;
use timelex_auth::{generate_keypair, WorkloadSigner, WorkloadVerifier};
use timelex_identity::{TenantId, UserId};
use timelex_mcp_host::{LeaseError, LeasePurpose, VaultKeySource};
use uuid::Uuid;

/// What the stub saw, so a test can check the request as well as the reply.
#[derive(Default, Clone)]
struct Seen {
    method: Option<Method>,
    uri: Option<Uri>,
    bearer: Option<String>,
}

struct Stub {
    verifier: WorkloadVerifier,
    seen: Arc<Mutex<Seen>>,
    answer: Answer,
}

#[derive(Clone)]
enum Answer {
    /// A 200 with this body.
    Body(String),
    /// This status with a problem body.
    Status(StatusCode),
}

fn keypair_auth() -> (String, String) {
    let kp = generate_keypair().unwrap();
    (kp.public_base64, kp.private_pkcs8_base64)
}

async fn stub(answer: Answer, public: &str) -> (String, Arc<Mutex<Seen>>, WorkloadVerifier) {
    let verifier = WorkloadVerifier::from_config("vault", &format!("mcp-host:mh1={public}")).unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let state = Arc::new(Stub {
        verifier: WorkloadVerifier::from_config("vault", &format!("mcp-host:mh1={public}")).unwrap(),
        seen: seen.clone(),
        answer,
    });
    let app = Router::new()
        .route("/internal/v1/secrets/{kind}/lease", post(lease))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen, verifier)
}

async fn lease(
    State(state): State<Arc<Stub>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    {
        let mut seen = state.seen.lock().unwrap();
        seen.method = Some(method);
        seen.uri = Some(uri);
        seen.bearer = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_owned);
    }
    let token = state
        .seen
        .lock()
        .unwrap()
        .bearer
        .clone()
        .expect("the client must present a bearer token");
    // A reply is only sent for a token the real vault would accept.
    if state.verifier.verify(&token).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "type": "urn:timelex:problem:unauthenticated" })),
        )
            .into_response();
    }
    match &state.answer {
        // The body is sent verbatim so a test can present a malformed reply.
        Answer::Body(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body.clone(),
        )
            .into_response(),
        Answer::Status(s) => (*s, Json(json!({ "type": "urn:timelex:problem:test" }))).into_response(),
    }
}

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4())
}

fn user() -> UserId {
    UserId::from_uuid(Uuid::new_v4())
}

/// A credential for tests: shaped like the real thing, built at run time.
fn value() -> String {
    format!("sk-ant-api03-{}", Uuid::new_v4().simple())
}

fn source(url: &str, private_key: &str) -> VaultKeySource {
    VaultKeySource::new(
        url,
        WorkloadSigner::from_base64("mcp-host", "mh1", private_key).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn a_lease_sends_the_scope_and_returns_the_key_with_its_expiry() {
    let secret = value();
    let expires = OffsetDateTime::from_unix_timestamp(1_800_000_060).unwrap();
    let body = json!({
        "kind": "anthropic_api_key",
        "value": secret.clone(),
        "expiresAt": expires.format(&time::format_description::well_known::Rfc3339).unwrap(),
    })
    .to_string();
    let (public, private) = keypair_auth();
    let (url, seen, verifier) = stub(Answer::Body(body), &public).await;
    let source = source(&url, &private);
    let t = tenant();
    let u = user();

    let lease = source.lease(t, u, LeasePurpose::ModelRun).await.unwrap();
    assert_eq!(lease.value().expose_secret(), secret);
    assert_eq!(lease.expires_at(), expires);
    assert_eq!(lease.kind().as_str(), "anthropic_api_key");
    // The value must not be printable through Debug.
    let shown = format!("{lease:?}");
    assert!(!shown.contains(&secret), "{shown}");
    assert!(shown.contains("****"));

    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.method, Some(Method::POST));
    assert_eq!(
        seen.uri.unwrap().path(),
        "/internal/v1/secrets/anthropic_api_key/lease"
    );
    let token = seen.bearer.unwrap();
    let verified = verifier.verify(&token).unwrap();
    assert_eq!(verified.service, "mcp-host");
    assert_eq!(verified.tenant, t);
    assert_eq!(verified.user, u);
    assert_eq!(verified.purpose, "model_run");
}

#[tokio::test]
async fn every_failure_is_one_closed_class_with_no_provider_words() {
    let cases = [
        (StatusCode::UNAUTHORIZED, LeaseError::Unauthorized),
        (StatusCode::FORBIDDEN, LeaseError::Forbidden),
        (StatusCode::NOT_FOUND, LeaseError::NotFound),
        (StatusCode::REQUEST_TIMEOUT, LeaseError::Unavailable),
        (StatusCode::TOO_MANY_REQUESTS, LeaseError::Unavailable),
        (StatusCode::INTERNAL_SERVER_ERROR, LeaseError::Unavailable),
        (StatusCode::BAD_GATEWAY, LeaseError::Unavailable),
        (StatusCode::IM_A_TEAPOT, LeaseError::Unusable),
    ];
    let (public, private) = keypair_auth();
    for (status, expected) in cases {
        let (url, _, _) = stub(Answer::Status(status), &public).await;
        let err = source(&url, &private)
            .lease(tenant(), user(), LeasePurpose::ModelRun)
            .await
            .unwrap_err();
        assert_eq!(err, expected, "{status}");
        // The database's own text must not ride on the error.
        let shown = format!("{err:?} {err}");
        assert!(!shown.contains("problem"), "{status}: {shown}");
    }
}

#[tokio::test]
async fn a_malformed_or_mismatched_reply_is_unusable() {
    let (public, private) = keypair_auth();
    let bodies = [
        "not json".to_string(),
        json!({ "kind": "anthropic_api_key" }).to_string(),
        json!({ "kind": "anthropic_api_key", "value": value() }).to_string(),
        json!({ "kind": "anthropic_api_key", "value": value(), "expiresAt": "tomorrow" }).to_string(),
        json!({ "kind": "anthropic_api_key", "value": "short", "expiresAt": "2027-01-15T08:00:00Z" })
            .to_string(),
        // A kind other than the one asked for is never accepted.
        json!({ "kind": "m365_oauth", "value": value(), "expiresAt": "2027-01-15T08:00:00Z" }).to_string(),
        format!(
            r#"{{"kind":"anthropic_api_key","value":"{}","expiresAt":"2027-01-15T08:00:00Z"}}"#,
            "x".repeat(5000)
        ),
    ];
    for body in bodies {
        let (url, _, _) = stub(Answer::Body(body.clone()), &public).await;
        assert_eq!(
            source(&url, &private)
                .lease(tenant(), user(), LeasePurpose::ModelRun)
                .await
                .unwrap_err(),
            LeaseError::Unusable,
            "{body}"
        );
    }
}

#[tokio::test]
async fn an_unreachable_vault_is_unavailable() {
    // A port that nothing listens on: bind, read the address, drop the listener.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let (_, private) = keypair_auth();
    assert_eq!(
        source(&url, &private)
            .lease(tenant(), user(), LeasePurpose::ModelRun)
            .await
            .unwrap_err(),
        LeaseError::Unavailable
    );
}

#[tokio::test]
async fn the_source_refuses_a_base_url_that_is_not_http() {
    let (_, private) = keypair_auth();
    let signer = WorkloadSigner::from_base64("mcp-host", "mh1", &private).unwrap();
    assert!(VaultKeySource::new("vault:8082", signer).is_err());
    let signer = WorkloadSigner::from_base64("mcp-host", "mh1", &private).unwrap();
    let shown = format!(
        "{:?}",
        VaultKeySource::new("http://127.0.0.1:8082", signer).unwrap()
    );
    assert!(!shown.contains(&private), "{shown}");
}
