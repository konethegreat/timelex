// Written by Kone Tshivhinda & Claude
//! End to end: gateway -> identity service (over real HTTP) -> Postgres with production roles.
//! P1-04 part a acceptance: "an authenticated call routes to a service", plus the security properties
//! that only show up when the pieces are joined.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use timelex_auth::{generate_keypair, Caller, Signer, Verifier};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_db::TenantPool;
use timelex_gateway::config::{sha256, Config};
use timelex_identity_service::{http as identity_http, IdentityStore, MIGRATIONS as IDENTITY_MIGRATIONS};
use timelex_vault::LocalKeyWrapper;
use timelex_vault_service::validator::{CheckResult, ProviderValidator};
use timelex_vault_service::{http as vault_http, LoadedSecret, VaultStore, MIGRATIONS as VAULT_MIGRATIONS};
use tower::ServiceExt;
use uuid::Uuid;

/// Random per run: no credential-shaped literal in the source, and no two runs share one.
static OPERATOR_KEY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("operator-{}", Uuid::new_v4().simple()));

/// The provider check as a script: every secret is accepted, so no test talks to the internet.
struct Accepted;

#[async_trait]
impl ProviderValidator for Accepted {
    async fn check(&self, _secret: &LoadedSecret) -> CheckResult {
        CheckResult::Accepted
    }
}

/// A vault master key built at run time, so no key is a literal in this repository.
fn fresh_master_key() -> [u8; 32] {
    let a = Uuid::new_v4().into_bytes();
    let b = Uuid::new_v4().into_bytes();
    std::array::from_fn(|i| if i < 16 { a[i] } else { b[i - 16] })
}

struct World {
    db: TestDb,
    gateway: Router,
    identity_url: String,
    vault_url: String,
    /// The gateway's private key, so a test can build a second gateway identity still trusts.
    private_key: String,
}

/// Build a gateway for this world, with the given firm budget.
fn build_gateway(
    private_key: &str,
    identity_url: &str,
    vault_url: &str,
    per_minute: u32,
    burst: u32,
) -> timelex_gateway::Gateway {
    timelex_gateway::app_with(Config {
        signer: Signer::from_base64("k1", private_key).unwrap(),
        operator_key_hashes: vec![sha256(OPERATOR_KEY.as_str())],
        service_urls: HashMap::from([
            ("identity".to_owned(), identity_url.to_owned()),
            ("vault".to_owned(), vault_url.to_owned()),
        ]),
        upstream_timeout: Duration::from_secs(5),
        rate_limit_per_minute: per_minute,
        rate_limit_burst: burst,
        internal_addr: "127.0.0.1:8089".parse().unwrap(),
    })
    .unwrap()
}

async fn world() -> Option<World> {
    let db = setup_db(&ServiceUnderTest {
        service: "identity",
        migrations: &IDENTITY_MIGRATIONS,
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

    // Vault: its own schema and role in the same throwaway database, migrated exactly as production
    // is, and trusting only the same gateway key for the `vault` audience.
    db.provision("vault").await;
    let migrator = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            db.options_for("timelex_migrator")
                .options([("search_path", "vault")]),
        )
        .await
        .unwrap();
    VAULT_MIGRATIONS.run(&migrator).await.unwrap();
    migrator.close().await;
    let vault_pool = TenantPool::from_pool(db.as_role("svc_vault").await);
    let vault_verifier = Verifier::from_config("vault", &format!("k1={}", kp.public_base64)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let vault_url = format!("http://{}", listener.local_addr().unwrap());
    let vault = vault_http::router(
        VaultStore::new(vault_pool, Arc::new(LocalKeyWrapper::new(fresh_master_key()))),
        vault_verifier,
        Arc::new(Accepted),
    );
    tokio::spawn(async move { axum::serve(listener, vault).await.unwrap() });

    // Gateway: holds the private key and the operator key's hash.
    let gateway = build_gateway(&kp.private_pkcs8_base64, &identity_url, &vault_url, 600, 120);
    Some(World {
        db,
        gateway: gateway.public,
        identity_url,
        vault_url,
        private_key: kp.private_pkcs8_base64,
    })
}

struct Reply {
    status: StatusCode,
    json: Value,
}

async fn post(app: &Router, path: &str, key: &str, body: Option<Value>) -> Reply {
    send(app, Method::POST, path, key, body).await
}

async fn send(app: &Router, method: Method, path: &str, key: &str, body: Option<Value>) -> Reply {
    let mut req = Request::builder()
        .method(method)
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

/// A tenant-key GET that must succeed, parsed as JSON.
async fn get_as(app: &Router, path: &str, key: &str) -> Value {
    let r = send(app, Method::GET, path, key, None).await;
    assert_eq!(r.status, StatusCode::OK, "GET {path} as a tenant: {}", r.status);
    r.json
}

fn firm(slug: &str) -> Value {
    json!({"slug": slug, "displayName": "Example Attorneys Inc", "adminEmail": format!("partner@{slug}.example"), "adminDisplayName": "Pat Partner"})
}

async fn get(app: &Router, path: &str) -> Value {
    let r = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = r.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    assert_eq!(
        parts.status,
        StatusCode::OK,
        "GET {path}: {}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
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
    let broken = build_gateway(&kp.private_pkcs8_base64, &dead_url, &w.vault_url, 600, 120).public;
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

#[tokio::test]
async fn a_firm_over_its_budget_is_refused_and_the_refusal_is_metered() {
    let Some(w) = world().await else { return };
    let a = create_firm(&w, "firm-a").await;
    let b = create_firm(&w, "firm-b").await;
    let key_a = a["initialApiKey"]["token"].as_str().unwrap();
    let key_b = b["initialApiKey"]["token"].as_str().unwrap();
    let tenant_a = a["tenantId"].as_str().unwrap().to_owned();

    // One token each, so the real identity service is in the loop for exactly the calls it should see.
    let tight = build_gateway(&w.private_key, &w.identity_url, &w.vault_url, 60, 1);
    let issue = || json!({"name": "k", "scopes": ["time:read"]});
    let r = post(&tight.public, "/v1/api-keys", key_a, Some(issue())).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    let r = post(&tight.public, "/v1/api-keys", key_a, Some(issue())).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(r.json["detail"].as_str().unwrap().contains("retry"));

    // Another firm still has its own budget.
    let r = post(&tight.public, "/v1/api-keys", key_b, Some(issue())).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);

    let usage = get(&tight.internal, "/internal/usage").await;
    assert_eq!(usage["totals"]["requests"], 3);
    assert_eq!(usage["totals"]["rateLimited"], 1);
    assert_eq!(usage["tenants"][&tenant_a]["requests"], 2);
    assert_eq!(usage["tenants"][&tenant_a]["rateLimited"], 1);
    assert_eq!(usage["tenants"][b["tenantId"].as_str().unwrap()]["requests"], 1);
    w.db.cleanup().await;
}

/// The value and its replacement are built at run time, so no credential-shaped literal is in the
/// source, and the vault's shape check (an Anthropic key starts with `sk-ant-`) still holds.
fn anthropic_value() -> String {
    format!("sk-ant-api03-{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn a_firm_manages_its_provider_key_through_the_gateway() {
    let Some(w) = world().await else { return };
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap().to_owned();

    // Store: the vault keeps the value and answers with metadata only.
    let value = anthropic_value();
    let r = send(
        &w.gateway,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        &admin_key,
        Some(json!({"value": value, "label": "billing workspace"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "PUT /v1/secrets: {}", r.status);
    let id = r.json["id"].as_str().unwrap().to_owned();
    assert_eq!(r.json["kind"], "anthropic_api_key");
    assert_eq!(r.json["label"], "billing workspace");
    assert_eq!(r.json["lastFour"], &value[value.len() - 4..]);
    assert!(
        !r.json.to_string().contains(&value),
        "the create response carried the value"
    );

    // A second PUT of the same kind replaces it, as the contract says.
    let replacement = anthropic_value();
    let r = send(
        &w.gateway,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        &admin_key,
        Some(json!({"value": replacement})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "PUT replaces: {}", r.status);
    assert_eq!(r.json["id"], id, "replaced under the same id");

    // List: metadata only, and never a value.
    let list = get_as(&w.gateway, "/v1/secrets", &admin_key).await;
    assert_eq!(list["secrets"].as_array().unwrap().len(), 1);
    assert_eq!(list["secrets"][0]["id"], id);
    assert!(
        !list.to_string().contains(&replacement),
        "the list carried the value"
    );

    // Test: the injected provider check accepts it.
    let r = post(&w.gateway, &format!("/v1/secrets/{id}/test"), &admin_key, None).await;
    assert_eq!(r.status, StatusCode::OK, "test: {}", r.status);
    assert_eq!(r.json["ok"], true);

    // Rotate: a new value, same id, metadata only.
    let next = anthropic_value();
    let r = post(
        &w.gateway,
        &format!("/v1/secrets/{id}/rotate"),
        &admin_key,
        Some(json!({"value": next})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "rotate: {}", r.status);
    assert_eq!(r.json["id"], id);
    assert!(!r.json.to_string().contains(&next), "rotate carried the value");

    // Revoke: gone from the default list, still visible when asked for, and no longer testable.
    let r = send(
        &w.gateway,
        Method::DELETE,
        &format!("/v1/secrets/{id}"),
        &admin_key,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(get_as(&w.gateway, "/v1/secrets", &admin_key).await["secrets"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        get_as(&w.gateway, "/v1/secrets?includeRevoked=true", &admin_key).await["secrets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let r = post(&w.gateway, &format!("/v1/secrets/{id}/test"), &admin_key, None).await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    // A key without `settings:manage` cannot see the firm's secrets, let alone change them.
    let r = post(
        &w.gateway,
        "/v1/api-keys",
        &admin_key,
        Some(json!({"name": "reporting", "scopes": ["time:read"]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    let narrow = r.json["token"].as_str().unwrap().to_owned();
    let r = send(&w.gateway, Method::GET, "/v1/secrets", &narrow, None).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    w.db.cleanup().await;
}

#[tokio::test]
async fn firms_cannot_touch_each_others_secrets_through_the_gateway() {
    let Some(w) = world().await else { return };
    let a = create_firm(&w, "firm-a").await;
    let b = create_firm(&w, "firm-b").await;
    let key_a = a["initialApiKey"]["token"].as_str().unwrap().to_owned();
    let key_b = b["initialApiKey"]["token"].as_str().unwrap().to_owned();

    let r = send(
        &w.gateway,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        &key_a,
        Some(json!({"value": anthropic_value()})),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.status);
    let id_a = r.json["id"].as_str().unwrap().to_owned();

    // B sees none of A's secrets, and A's secret looks like it does not exist.
    assert!(get_as(&w.gateway, "/v1/secrets", &key_b).await["secrets"]
        .as_array()
        .unwrap()
        .is_empty());
    for (method, path, body) in [
        (Method::DELETE, format!("/v1/secrets/{id_a}"), None),
        (Method::POST, format!("/v1/secrets/{id_a}/test"), None),
        (
            Method::POST,
            format!("/v1/secrets/{id_a}/rotate"),
            Some(json!({"value": anthropic_value()})),
        ),
    ] {
        let r = send(&w.gateway, method, &path, &key_b, body).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}");
    }

    // A's own view is unchanged.
    assert_eq!(
        get_as(&w.gateway, "/v1/secrets", &key_a).await["secrets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    w.db.cleanup().await;
}

#[tokio::test]
async fn the_vault_refuses_anything_that_did_not_come_from_the_gateway() {
    let Some(w) = world().await else { return };
    let created = create_firm(&w, "example-attorneys").await;
    let admin_key = created["initialApiKey"]["token"].as_str().unwrap();
    let client = reqwest::Client::new();
    let url = format!("{}/v1/secrets", w.vault_url);

    // A tenant API key presented straight to the vault: not an internal token.
    let status = client
        .get(&url)
        .bearer_auth(admin_key)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A well-formed token for the right audience, signed by someone else's key.
    let attacker = Signer::from_base64("k1", &generate_keypair().unwrap().private_pkcs8_base64).unwrap();
    let forged = attacker.sign("vault", &Caller::Operator).unwrap();
    let status = client
        .get(&url)
        .bearer_auth(forged)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Even the gateway's own key earns 403 for an operator token: only a principal may touch secrets.
    let operator = Signer::from_base64("k1", &w.private_key)
        .unwrap()
        .sign("vault", &Caller::Operator)
        .unwrap();
    let status = client
        .get(&url)
        .bearer_auth(operator)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::FORBIDDEN);
    w.db.cleanup().await;
}
