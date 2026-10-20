// Written by Kone Tshivhinda & Claude
//! End to end: gateway -> identity service (over real HTTP) -> Postgres with production roles.
//! P1-04 part a acceptance: "an authenticated call routes to a service", plus the security properties
//! that only show up when the pieces are joined.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use timelex_auth::{generate_keypair, Caller, Signer, Verifier};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_gateway::config::{sha256, Config};
use timelex_identity_service::{http as identity_http, IdentityStore, MIGRATIONS};
use tower::ServiceExt;
use uuid::Uuid;

/// Random per run: no credential-shaped literal in the source, and no two runs share one.
static OPERATOR_KEY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("operator-{}", Uuid::new_v4().simple()));

struct World {
    db: TestDb,
    gateway: Router,
    identity_url: String,
}

async fn world() -> Option<World> {
    let db = setup_db(&ServiceUnderTest {
        service: "identity",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let kp = generate_keypair().unwrap();

    // Identity: trusts only the gateway's public key, listens on an ephemeral loopback port.
    let verifier = Verifier::from_config("identity", &format!("k1={}", kp.public_base64)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let identity_url = format!("http://{}", listener.local_addr().unwrap());
    let identity = identity_http::router(IdentityStore::new(tenants), verifier);
    tokio::spawn(async move { axum::serve(listener, identity).await.unwrap() });

    // Gateway: holds the private key and the operator key's hash.
    let gateway = timelex_gateway::app_with(Config {
        signer: Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
        operator_key_hashes: vec![sha256(OPERATOR_KEY.as_str())],
        identity_url: identity_url.clone(),
        upstream_timeout: std::time::Duration::from_secs(5),
    })
    .unwrap();
    Some(World {
        db,
        gateway,
        identity_url,
    })
}

struct Reply {
    status: StatusCode,
    json: Value,
}

async fn post(app: &Router, path: &str, key: &str, body: Option<Value>) -> Reply {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {key}"));
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let r = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let (parts, body) = r.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    Reply {
        status: parts.status,
        json: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

fn firm(slug: &str) -> Value {
    json!({"slug": slug, "displayName": "Example Attorneys Inc", "adminEmail": format!("partner@{slug}.example"), "adminDisplayName": "Pat Partner"})
}

async fn create_firm(w: &World, slug: &str) -> Value {
    let r = post(&w.gateway, "/v1/tenants", OPERATOR_KEY.as_str(), Some(firm(slug))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    r.json
}

#[tokio::test]
async fn the_whole_journey_through_the_gateway() {
    let Some(w) = world().await else { return };

    // 1. A platform operator creates a firm and receives its first key.
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap().to_owned();

    // 2. The firm's admin issues a narrow key for reporting.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        &admin_key,
        Some(json!({"name": "reporting", "scopes": ["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    let narrow_key = r.json["token"].as_str().unwrap().to_owned();
    let narrow_id = r.json["id"].as_str().unwrap().to_owned();

    // 3. That narrow key authenticates but is not allowed to issue keys: its token says so.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        &narrow_key,
        Some(json!({"name": "escalation", "scopes": ["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);

    // 4. The admin revokes it; the very next call with it is refused (nothing is cached).
    let r = post(
        &w.gateway,
        &format!("/v1/api-keys/{narrow_id}/revoke"),
        &admin_key,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        &narrow_key,
        Some(json!({"name": "again", "scopes": ["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    // 5. The admin's own key is unaffected.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        &admin_key,
        Some(json!({"name": "second", "scopes": ["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    w.db.cleanup().await;
}

#[tokio::test]
async fn firms_cannot_touch_each_other_through_the_gateway() {
    let Some(w) = world().await else { return };
    let a = create_firm(&w, "firm-a").await;
    let b = create_firm(&w, "firm-b").await;
    let key_a = a["initialApiKey"]["token"].as_str().unwrap();
    let key_b = b["initialApiKey"]["token"].as_str().unwrap();

    // A cannot revoke B's key, nor issue keys for B's user: both look like "not found".
    let r = post(
        &w.gateway,
        &format!(
            "/v1/api-keys/{}/revoke",
            b["initialApiKey"]["id"].as_str().unwrap()
        ),
        key_a,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        key_a,
        Some(json!({"name":"k","scopes":["time:read"],"userId": b["adminUserId"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // A request cannot name its tenant: unknown fields are refused, so there is nowhere to put one.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        key_a,
        Some(json!({"name":"k","scopes":["time:read"],"tenantId": b["tenantId"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // B's key still works.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        key_b,
        Some(json!({"name":"k","scopes":["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    w.db.cleanup().await;
}

#[tokio::test]
async fn a_tenant_key_cannot_create_firms_and_the_operator_key_cannot_act_as_a_tenant() {
    let Some(w) = world().await else { return };
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap();
    let r = post(&w.gateway, "/v1/tenants", admin_key, Some(firm("second-firm"))).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "even a firm admin is not an operator"
    );
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        OPERATOR_KEY.as_str(),
        Some(json!({"name":"k","scopes":["time:read"]})),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "the operator key is not a tenant key"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.tenants")
        .fetch_one(&w.db.admin)
        .await
        .unwrap();
    assert_eq!(n, 1);
    w.db.cleanup().await;
}

#[tokio::test]
async fn the_identity_service_refuses_anything_that_did_not_come_from_the_gateway() {
    let Some(w) = world().await else { return };
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap();
    let client = reqwest::Client::new();
    let direct = |token: String| {
        let client = client.clone();
        let url = format!("{}/v1/api-keys", w.identity_url);
        async move {
            client
                .post(url)
                .bearer_auth(token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(json!({"name":"k","scopes":["time:read"]}).to_string())
                .send()
                .await
                .unwrap()
                .status()
        }
    };
    // A tenant API key presented straight to identity: not an internal token.
    assert_eq!(direct(admin_key.to_owned()).await, StatusCode::UNAUTHORIZED);
    // The operator key presented straight to identity: also not an internal token.
    assert_eq!(
        direct(OPERATOR_KEY.as_str().to_owned()).await,
        StatusCode::UNAUTHORIZED
    );
    // A well-formed internal token signed by someone else's key (an attacker who can reach identity).
    let attacker = Signer::from_base64("k1", &generate_keypair().unwrap().private_pkcs8_base64).unwrap();
    let forged = attacker.sign("identity", &Caller::Operator).unwrap();
    assert_eq!(direct(forged).await, StatusCode::UNAUTHORIZED);
    w.db.cleanup().await;
}

#[tokio::test]
async fn a_stopped_identity_service_makes_the_gateway_fail_closed() {
    // A gateway pointed at a service that accepts then drops connections never lets a call through.
    let Some(w) = world().await else { return };
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap();
    let dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_url = format!("http://{}", dead.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let _ = dead.accept().await;
        }
    });
    let kp = generate_keypair().unwrap();
    let broken = timelex_gateway::app_with(Config {
        signer: Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
        operator_key_hashes: vec![sha256(OPERATOR_KEY.as_str())],
        identity_url: dead_url,
        upstream_timeout: std::time::Duration::from_secs(2),
    })
    .unwrap();
    let r = post(
        &broken,
        "/v1/api-keys",
        admin_key,
        Some(json!({"name":"k","scopes":["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    w.db.cleanup().await;
}
