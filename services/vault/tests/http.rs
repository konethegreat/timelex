// Written by Kone Tshivhinda & Claude
//! The vault HTTP layer against `contracts/openapi/vault.v1.yaml`, with a real Postgres behind it.
//! Callers are simulated by signing internal tokens exactly as the gateway does; the provider check
//! is a script, so no test talks to the internet.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use timelex_auth::{generate_keypair, Caller, Principal, Signer, Verifier};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use timelex_vault::LocalKeyWrapper;
use timelex_vault_service::validator::{CheckResult, ProviderValidator};
use timelex_vault_service::{http, LoadedSecret, VaultStore, MIGRATIONS};
use tower::ServiceExt;
use uuid::Uuid;

struct Env {
    db: TestDb,
    app: Router,
    signer: Signer,
}

/// The provider check as a script: every secret gets the same verdict.
struct Fixed(CheckResult);

#[async_trait]
impl ProviderValidator for Fixed {
    async fn check(&self, _secret: &LoadedSecret) -> CheckResult {
        self.0
    }
}

/// A fresh master key per test, so no key is a literal in this repository.
fn random_key() -> [u8; 32] {
    let bytes: Vec<u8> = (0..2).flat_map(|_| Uuid::new_v4().into_bytes()).collect();
    std::array::from_fn(|i| bytes[i])
}

async fn env_with(result: CheckResult) -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "vault",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let kp = generate_keypair().unwrap();
    let signer = Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap();
    let verifier = Verifier::from_config("vault", &format!("k1={}", kp.public_base64)).unwrap();
    let store = VaultStore::new(tenants, Arc::new(LocalKeyWrapper::new(random_key())));
    let app = http::router(store, verifier, Arc::new(Fixed(result)));
    Some(Env { db, app, signer })
}

async fn env() -> Option<Env> {
    env_with(CheckResult::Accepted).await
}

struct Reply {
    status: StatusCode,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
}

async fn send(app: &Router, method: Method, path: &str, bearer: Option<&str>, body: Option<String>) -> Reply {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b)
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    Reply {
        status: parts.status,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4())
}

fn principal(tenant: TenantId, permissions: &[Permission]) -> Principal {
    Principal {
        tenant,
        user: UserId::from_uuid(Uuid::new_v4()),
        key: ApiKeyId::from_uuid(Uuid::new_v4()),
        permissions: permissions.iter().copied().collect(),
    }
}

fn settings(tenant: TenantId) -> Principal {
    principal(tenant, &[Permission::SettingsManage])
}

fn token(env: &Env, principal: &Principal) -> String {
    env.signer
        .sign("vault", &Caller::Principal(principal.clone()))
        .unwrap()
}

/// A credential for tests: shaped like the real thing, built at run time.
fn value() -> Value {
    Value::String(format!("sk-ant-api03-{}", Uuid::new_v4().simple()))
}

async fn put_secret(env: &Env, bearer: &str, value: &Value) -> Reply {
    let body = json!({ "value": value, "label": "Firm Anthropic key" }).to_string();
    send(
        &env.app,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        Some(bearer),
        Some(body),
    )
    .await
}

/// The last four characters of an ASCII value, which is what the API is allowed to show.
fn last_four(value: &Value) -> String {
    let s = value.as_str().unwrap();
    s[s.len() - 4..].to_owned()
}

/// Create one secret and return its metadata JSON.
async fn stored(env: &Env, bearer: &str, value: &Value) -> Value {
    let r = put_secret(env, bearer, value).await;
    assert_eq!(r.status, StatusCode::CREATED);
    r.json()
}

fn id_of(metadata: &Value) -> &str {
    metadata["id"].as_str().unwrap()
}

async fn listed(env: &Env, bearer: &str, query: &str) -> Value {
    let r = send(
        &env.app,
        Method::GET,
        &format!("/v1/secrets{query}"),
        Some(bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    r.json()
}

// ---------- who may call ----------

#[tokio::test]
async fn health_needs_no_credentials() {
    let Some(env) = env().await else { return };
    let r = send(&env.app, Method::GET, "/health", None, None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["status"], "ok");
    env.db.cleanup().await;
}

#[tokio::test]
async fn every_unusable_credential_gets_the_same_401() {
    let Some(env) = env().await else { return };
    let other = generate_keypair().unwrap();
    let attacker = Signer::from_base64("k1", &other.private_pkcs8_base64).unwrap();
    let bad: Vec<(&str, Option<String>)> = vec![
        ("no header", None),
        ("garbage", Some("garbage".into())),
        (
            "signed by another key",
            Some(
                attacker
                    .sign("vault", &Caller::Principal(settings(tenant())))
                    .unwrap(),
            ),
        ),
    ];
    for (why, bearer) in bad {
        let r = send(&env.app, Method::GET, "/v1/secrets", bearer.as_deref(), None).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{why}");
        assert_eq!(r.json()["type"], "urn:timelex:problem:unauthenticated", "{why}");
    }
    env.db.cleanup().await;
}

#[tokio::test]
async fn only_a_principal_with_settings_manage_may_touch_secrets() {
    let Some(env) = env().await else { return };
    let t = tenant();
    let body = json!({ "value": value() }).to_string();
    let cases = [
        ("an operator token", Caller::Operator, StatusCode::FORBIDDEN),
        (
            "a principal without the permission",
            Caller::Principal(principal(t, &[Permission::TimeRead])),
            StatusCode::FORBIDDEN,
        ),
    ];
    for (why, caller, expected) in cases {
        let bearer = env.signer.sign("vault", &caller).unwrap();
        let r = send(
            &env.app,
            Method::PUT,
            "/v1/secrets/anthropic_api_key",
            Some(&bearer),
            Some(body.clone()),
        )
        .await;
        assert_eq!(r.status, expected, "{why}");
    }
    // The same call with `settings:manage` works, so the refusals above were about permission.
    let bearer = token(&env, &settings(t));
    let r = send(
        &env.app,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        Some(&bearer),
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    env.db.cleanup().await;
}

// ---------- the round trip ----------

#[tokio::test]
async fn a_secret_is_created_listed_and_revoked_over_http() {
    let Some(env) = env().await else { return };
    let bearer = token(&env, &settings(tenant()));
    let secret = value();
    let metadata = stored(&env, &bearer, &secret).await;

    assert_eq!(metadata["kind"], "anthropic_api_key");
    assert_eq!(metadata["label"], "Firm Anthropic key");
    assert_eq!(metadata["allowedHosts"], json!(["api.anthropic.com"]));
    assert_eq!(metadata["lastFour"], last_four(&secret));
    assert!(metadata.get("value").is_none(), "metadata must not carry a value");
    assert!(metadata.get("lastTestedAt").is_none());
    assert!(metadata.get("lastUsedAt").is_none());
    assert!(metadata.get("revokedAt").is_none());

    let list = listed(&env, &bearer, "").await;
    assert_eq!(list["secrets"].as_array().unwrap().len(), 1);

    let id = id_of(&metadata).to_owned();
    let r = send(
        &env.app,
        Method::DELETE,
        &format!("/v1/secrets/{id}"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);

    assert!(listed(&env, &bearer, "").await["secrets"]
        .as_array()
        .unwrap()
        .is_empty());
    let revoked = listed(&env, &bearer, "?includeRevoked=true").await;
    assert!(revoked["secrets"][0]["revokedAt"].is_string());

    // Revoking twice is the same 204: a retried request cannot tell it apart.
    let r = send(
        &env.app,
        Method::DELETE,
        &format!("/v1/secrets/{id}"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    env.db.cleanup().await;
}

#[tokio::test]
async fn putting_the_same_kind_again_is_a_replacement_under_the_same_id() {
    let Some(env) = env().await else { return };
    let bearer = token(&env, &settings(tenant()));
    let first = stored(&env, &bearer, &value()).await;

    let second = put_secret(&env, &bearer, &value()).await;
    assert_eq!(second.status, StatusCode::OK);
    let second = second.json();
    assert_eq!(id_of(&first), id_of(&second));
    assert_ne!(first["lastFour"], second["lastFour"]);
    assert_eq!(
        listed(&env, &bearer, "").await["secrets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn no_response_ever_contains_the_value() {
    let Some(env) = env_with(CheckResult::Accepted).await else {
        return;
    };
    let bearer = token(&env, &settings(tenant()));
    let secret = value();
    let metadata = stored(&env, &bearer, &secret).await;
    let id = id_of(&metadata).to_owned();

    let rotated = value();
    let replies = [
        ("create", put_secret(&env, &bearer, &secret).await),
        (
            "list",
            send(&env.app, Method::GET, "/v1/secrets", Some(&bearer), None).await,
        ),
        (
            "test",
            send(
                &env.app,
                Method::POST,
                &format!("/v1/secrets/{id}/test"),
                Some(&bearer),
                None,
            )
            .await,
        ),
        (
            "rotate",
            send(
                &env.app,
                Method::POST,
                &format!("/v1/secrets/{id}/rotate"),
                Some(&bearer),
                Some(json!({ "value": rotated }).to_string()),
            )
            .await,
        ),
    ];
    for (what, reply) in &replies {
        let text = &reply.text;
        assert!(
            !text.contains(secret.as_str().unwrap()),
            "the {what} response carried the value"
        );
        assert!(
            !text.contains(rotated.as_str().unwrap()),
            "the {what} response carried the rotated value"
        );
        // The last four characters are the advertised exception, so only the full value is checked.
        assert!(
            !text.contains("value\":\"sk-ant"),
            "the {what} response had a value-shaped field"
        );
    }
    env.db.cleanup().await;
}

// ---------- request validation ----------

#[tokio::test]
async fn a_malformed_body_is_400_and_an_oversized_one_is_413() {
    let Some(env) = env().await else { return };
    let bearer = token(&env, &settings(tenant()));
    let path = "/v1/secrets/anthropic_api_key";
    let bodies = [
        json!({ "value": 5 }).to_string(),
        json!({ "label": "no value" }).to_string(),
        json!({ "value": value(), "surprise": true }).to_string(),
        "not json at all".to_string(),
    ];
    for (i, body) in bodies.into_iter().enumerate() {
        let r = send(&env.app, Method::PUT, path, Some(&bearer), Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "body {i}");
        assert_eq!(
            r.json()["type"],
            "urn:timelex:problem:invalid-request",
            "body {i}"
        );
    }

    let huge = json!({ "value": "x".repeat(70 * 1024) }).to_string();
    let r = send(&env.app, Method::PUT, path, Some(&bearer), Some(huge)).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
    env.db.cleanup().await;
}

#[tokio::test]
async fn an_unknown_kind_a_host_outside_the_kind_and_a_bad_id_are_400() {
    let Some(env) = env().await else { return };
    let bearer = token(&env, &settings(tenant()));

    let r = send(
        &env.app,
        Method::PUT,
        "/v1/secrets/postgres_password",
        Some(&bearer),
        Some(json!({ "value": value() }).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let r = send(
        &env.app,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        Some(&bearer),
        Some(json!({ "value": value(), "allowedHosts": ["evil.example"] }).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["type"], "urn:timelex:problem:invalid-request");

    let r = send(
        &env.app,
        Method::GET,
        "/v1/secrets?includeRevoked=maybe",
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let r = send(
        &env.app,
        Method::DELETE,
        "/v1/secrets/not-a-uuid",
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    env.db.cleanup().await;
}

// ---------- the provider check ----------

#[tokio::test]
async fn a_passing_test_is_200_and_stamps_the_test_not_the_use() {
    let Some(env) = env_with(CheckResult::Accepted).await else {
        return;
    };
    let bearer = token(&env, &settings(tenant()));
    let metadata = stored(&env, &bearer, &value()).await;
    let id = id_of(&metadata).to_owned();

    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/test"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["ok"], true);
    assert!(r.json()["checkedAt"].is_string());

    let list = listed(&env, &bearer, "").await;
    let secret = &list["secrets"][0];
    assert_eq!(secret["lastTestedAt"], r.json()["checkedAt"]);
    assert!(secret.get("lastUsedAt").is_none(), "testing is not use");
    env.db.cleanup().await;
}

#[tokio::test]
async fn a_rejected_secret_is_422_and_the_test_is_recorded() {
    let Some(env) = env_with(CheckResult::Rejected).await else {
        return;
    };
    let bearer = token(&env, &settings(tenant()));
    let metadata = stored(&env, &bearer, &value()).await;
    let id = id_of(&metadata).to_owned();

    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/test"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.json()["type"], "urn:timelex:problem:provider-rejected");

    let list = listed(&env, &bearer, "").await;
    assert!(list["secrets"][0]["lastTestedAt"].is_string());
    env.db.cleanup().await;
}

#[tokio::test]
async fn an_unreachable_provider_is_502_and_no_test_is_recorded() {
    let Some(env) = env_with(CheckResult::Unreachable).await else {
        return;
    };
    let bearer = token(&env, &settings(tenant()));
    let metadata = stored(&env, &bearer, &value()).await;
    let id = id_of(&metadata).to_owned();

    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/test"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    assert_eq!(r.json()["type"], "urn:timelex:problem:provider-unreachable");

    // A provider outage must not look like a bad key.
    let list = listed(&env, &bearer, "").await;
    assert!(list["secrets"][0].get("lastTestedAt").is_none());
    env.db.cleanup().await;
}

// ---------- isolation and revocation ----------

#[tokio::test]
async fn another_firms_secret_is_a_404_for_every_operation() {
    let Some(env) = env().await else { return };
    let a = token(&env, &settings(tenant()));
    let b = token(&env, &settings(tenant()));
    let metadata = stored(&env, &a, &value()).await;
    let id = id_of(&metadata).to_owned();

    let calls = [
        (Method::POST, format!("/v1/secrets/{id}/test")),
        (Method::POST, format!("/v1/secrets/{id}/rotate")),
        (Method::DELETE, format!("/v1/secrets/{id}")),
    ];
    for (method, path) in calls {
        let body = (method == Method::POST && path.ends_with("rotate"))
            .then(|| json!({ "value": value() }).to_string());
        let r = send(&env.app, method.clone(), &path, Some(&b), body).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{method} {path}");
    }
    // The other firm never sees it either.
    assert!(listed(&env, &b, "?includeRevoked=true").await["secrets"]
        .as_array()
        .unwrap()
        .is_empty());
    env.db.cleanup().await;
}

#[tokio::test]
async fn rotating_keeps_the_id_and_a_revoked_secret_is_409() {
    let Some(env) = env().await else { return };
    let bearer = token(&env, &settings(tenant()));
    let first = stored(&env, &bearer, &value()).await;
    let id = id_of(&first).to_owned();

    let rotated = value();
    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/rotate"),
        Some(&bearer),
        Some(json!({ "value": rotated }).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let metadata = r.json();
    assert_eq!(id_of(&metadata), id);
    assert_eq!(metadata["label"], "Firm Anthropic key");

    let r = send(
        &env.app,
        Method::DELETE,
        &format!("/v1/secrets/{id}"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/rotate"),
        Some(&bearer),
        Some(json!({ "value": value() }).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/secrets/{id}/test"),
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    env.db.cleanup().await;
}
