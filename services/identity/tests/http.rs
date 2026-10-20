// Written by Kone Tshivhinda & Claude
//! The identity HTTP layer against `contracts/openapi/identity.v1.yaml`, with a real Postgres behind it.
//! Callers are simulated by signing internal tokens, exactly as the gateway does.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};
use timelex_auth::{generate_keypair, Caller, Principal, Signer, Verifier};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use timelex_identity_service::{http, IdentityStore, MIGRATIONS};
use tower::ServiceExt;
use uuid::Uuid;

struct Env {
    db: TestDb,
    app: Router,
    signer: Signer,
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "identity",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let kp = generate_keypair().unwrap();
    let signer = Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap();
    let verifier = Verifier::from_config("identity", &format!("k1={}", kp.public_base64)).unwrap();
    let app = http::router(IdentityStore::new(tenants), verifier);
    Some(Env { db, app, signer })
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
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
        headers: parts.headers,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

/// A well-formed but unknown tenant key, built at run time so no key-shaped literal sits in the source.
fn fake_key() -> String {
    let tenant = TenantId::from_uuid(Uuid::from_u128(0x0191a2b3c4d57e6f8a9b0c1d2e3f4a5b));
    timelex_identity::ApiKeyToken::new(tenant, [7u8; 32])
        .expose_secret()
        .to_owned()
}

fn tok(env: &Env, caller: &Caller) -> String {
    env.signer.sign("identity", caller).unwrap()
}

fn new_firm(slug: &str) -> String {
    json!({"slug": slug, "displayName": "Example Attorneys Inc", "adminEmail": format!("partner@{slug}.example"), "adminDisplayName": "Pat Partner"}).to_string()
}

/// Create a firm as an operator; return the response JSON and a principal for its first admin.
async fn make_firm(env: &Env, slug: &str) -> (Value, Principal) {
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&tok(env, &Caller::Operator)),
        Some(new_firm(slug)),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let j = r.json();
    let key = &j["initialApiKey"];
    let principal = Principal {
        tenant: TenantId::from_uuid(j["tenantId"].as_str().unwrap().parse().unwrap()),
        user: UserId::from_uuid(j["adminUserId"].as_str().unwrap().parse().unwrap()),
        key: ApiKeyId::from_uuid(key["id"].as_str().unwrap().parse().unwrap()),
        permissions: key["scopes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Permission::parse(s.as_str().unwrap()).unwrap())
            .collect(),
    };
    (j, principal)
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
    let long_ago = OffsetDateTime::now_utc() - Duration::hours(1);
    let bad: Vec<(&str, Option<String>)> = vec![
        ("no header", None),
        ("garbage", Some("garbage".into())),
        ("a tenant api key", Some(fake_key())),
        (
            "signed by another key",
            Some(attacker.sign("identity", &Caller::Operator).unwrap()),
        ),
        (
            "wrong audience",
            Some(env.signer.sign("vault", &Caller::Operator).unwrap()),
        ),
        (
            "expired",
            Some(
                env.signer
                    .sign_at("identity", &Caller::Operator, long_ago)
                    .unwrap(),
            ),
        ),
    ];
    let mut bodies = Vec::new();
    for (label, token) in bad {
        let r = send(
            &env.app,
            Method::POST,
            "/v1/tenants",
            token.as_deref(),
            Some(new_firm("example-attorneys")),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{label}");
        assert_eq!(r.headers[header::WWW_AUTHENTICATE], "Bearer", "{label}");
        assert_eq!(
            r.headers[header::CONTENT_TYPE],
            "application/problem+json",
            "{label}"
        );
        bodies.push(r.text);
    }
    assert!(
        bodies.windows(2).all(|w| w[0] == w[1]),
        "401 bodies differ: {bodies:?}"
    );
    // Nothing was created by any of those attempts.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.tenants")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(n, 0);
    env.db.cleanup().await;
}

#[tokio::test]
async fn a_valid_token_of_the_wrong_kind_is_a_403() {
    let Some(env) = env().await else { return };
    let (_, admin) = make_firm(&env, "example-attorneys").await;
    let principal = Caller::Principal(admin);
    let revoke_path = format!("/v1/api-keys/{}/revoke", Uuid::new_v4());
    let cases: Vec<(Method, &str, &Caller, Option<String>)> = vec![
        (
            Method::POST,
            "/v1/tenants",
            &principal,
            Some(new_firm("second-firm")),
        ),
        (
            Method::POST,
            "/v1/tenants",
            &Caller::Service,
            Some(new_firm("second-firm")),
        ),
        (
            Method::POST,
            "/v1/api-keys",
            &Caller::Operator,
            Some(json!({"name":"k","scopes":["time:read"]}).to_string()),
        ),
        (
            Method::POST,
            "/v1/api-keys",
            &Caller::Service,
            Some(json!({"name":"k","scopes":["time:read"]}).to_string()),
        ),
        (Method::POST, &revoke_path, &Caller::Operator, None),
        (
            Method::POST,
            "/v1/authenticate",
            &Caller::Operator,
            Some(json!({"token":"x"}).to_string()),
        ),
        (
            Method::POST,
            "/v1/authenticate",
            &principal,
            Some(json!({"token":"x"}).to_string()),
        ),
    ];
    for (method, path, caller, body) in cases {
        let r = send(&env.app, method, path, Some(&tok(&env, caller)), body).await;
        assert_eq!(
            r.status,
            StatusCode::FORBIDDEN,
            "{path} as {:?}: {}",
            caller.kind(),
            r.text
        );
    }
    env.db.cleanup().await;
}

// ---------- createTenant ----------

#[tokio::test]
async fn create_tenant_returns_the_contract_shape_and_forbids_caching() {
    let Some(env) = env().await else { return };
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&tok(&env, &Caller::Operator)),
        Some(new_firm("example-attorneys")),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    let j = r.json();
    assert_eq!(j["slug"], "example-attorneys");
    for field in ["tenantId", "adminUserId"] {
        assert!(Uuid::parse_str(j[field].as_str().unwrap()).is_ok(), "{field}");
    }
    let key = &j["initialApiKey"];
    assert_eq!(key["token"].as_str().unwrap().len(), 80);
    assert_eq!(key["prefix"].as_str().unwrap().len(), 8);
    assert_eq!(key["userId"], j["adminUserId"]);
    assert!(key["expiresAt"].is_null());
    assert!(key["createdAt"].as_str().unwrap().contains('T'));
    assert!(key["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "keys:manage"));
    env.db.cleanup().await;
}

#[tokio::test]
async fn create_tenant_rejects_bad_input_without_echoing_it() {
    let Some(env) = env().await else { return };
    let op = tok(&env, &Caller::Operator);
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&op),
        Some(new_firm("Bad Slug")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["type"], "urn:timelex:problem:invalid-request");

    let mut with_extra: Value = serde_json::from_str(&new_firm("example-attorneys")).unwrap();
    with_extra["isAdmin"] = json!(true);
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&op),
        Some(with_extra.to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "unknown fields are refused");

    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&op),
        Some("{not json".into()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let secret_looking = "tlx_SECRET-LOOKING-VALUE";
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&op),
        Some(json!({"slug": secret_looking, "displayName": 5}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        !r.text.contains("SECRET-LOOKING-VALUE"),
        "error body echoed the input: {}",
        r.text
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn a_duplicate_slug_is_a_409_and_a_huge_body_is_a_413() {
    let Some(env) = env().await else { return };
    let op = tok(&env, &Caller::Operator);
    make_firm(&env, "example-attorneys").await;
    let r = send(
        &env.app,
        Method::POST,
        "/v1/tenants",
        Some(&op),
        Some(new_firm("example-attorneys")),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let huge = json!({"slug": "x".repeat(70_000)}).to_string();
    let r = send(&env.app, Method::POST, "/v1/tenants", Some(&op), Some(huge)).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
    env.db.cleanup().await;
}

// ---------- issueApiKey / revokeApiKey ----------

#[tokio::test]
async fn an_admin_issues_and_revokes_keys_and_the_token_is_shown_once() {
    let Some(env) = env().await else { return };
    let (_, admin) = make_firm(&env, "example-attorneys").await;
    let t = tok(&env, &Caller::Principal(admin));

    let body = json!({"name": "reporting", "scopes": ["time:read", "billing:read"], "expiresAt": (OffsetDateTime::now_utc() + Duration::days(30)).format(&time::format_description::well_known::Rfc3339).unwrap()});
    let r = send(
        &env.app,
        Method::POST,
        "/v1/api-keys",
        Some(&t),
        Some(body.to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    let j = r.json();
    assert_eq!(j["token"].as_str().unwrap().len(), 80);
    let mut scopes: Vec<&str> = j["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    scopes.sort();
    assert_eq!(
        scopes,
        ["billing:read", "time:read"],
        "exactly the requested scopes, no duplicates"
    );
    assert!(j["expiresAt"].is_string());

    let revoke = format!("/v1/api-keys/{}/revoke", j["id"].as_str().unwrap());
    let r = send(&env.app, Method::POST, &revoke, Some(&t), None).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(r.text.is_empty());
    // Repeating is fine.
    assert_eq!(
        send(&env.app, Method::POST, &revoke, Some(&t), None).await.status,
        StatusCode::NO_CONTENT
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn issuing_checks_permissions_scopes_and_firm_boundaries() {
    let Some(env) = env().await else { return };
    let (_, admin_a) = make_firm(&env, "firm-a").await;
    let (jb, _) = make_firm(&env, "firm-b").await;
    let ta = tok(&env, &Caller::Principal(admin_a.clone()));

    // A token whose permissions lack keys:manage: refused, whatever the body says.
    let weak = Principal {
        permissions: [Permission::TimeRead].into_iter().collect(),
        ..admin_a.clone()
    };
    let r = send(
        &env.app,
        Method::POST,
        "/v1/api-keys",
        Some(&tok(&env, &Caller::Principal(weak))),
        Some(json!({"name":"k","scopes":["time:read"]}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // An unknown scope string, no scopes, a bad expiry format.
    for body in [
        json!({"name":"k","scopes":["root:everything"]}),
        json!({"name":"k","scopes":[]}),
        json!({"name":"k","scopes":["time:read"],"expiresAt":"tomorrow"}),
        json!({"name":"k","scopes":["time:read"],"userId":"not-a-uuid"}),
        json!({"name":"k","scopes":["time:read"],"tenantId": Uuid::new_v4()}),
    ] {
        let r = send(
            &env.app,
            Method::POST,
            "/v1/api-keys",
            Some(&ta),
            Some(body.to_string()),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}");
    }

    // An expiry in the past is a rule violation (400), not a server error.
    let past = (OffsetDateTime::now_utc() - Duration::days(1))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let r = send(
        &env.app,
        Method::POST,
        "/v1/api-keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["time:read"],"expiresAt":past}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Another firm's user (or key) is simply not found.
    let b_admin = jb["adminUserId"].as_str().unwrap();
    let r = send(
        &env.app,
        Method::POST,
        "/v1/api-keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["time:read"],"userId":b_admin}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let b_key = jb["initialApiKey"]["id"].as_str().unwrap();
    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/api-keys/{b_key}/revoke"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    env.db.cleanup().await;
}

#[tokio::test]
async fn revoke_validates_its_path_and_permission() {
    let Some(env) = env().await else { return };
    let (_, admin) = make_firm(&env, "example-attorneys").await;
    let t = tok(&env, &Caller::Principal(admin.clone()));
    assert_eq!(
        send(
            &env.app,
            Method::POST,
            "/v1/api-keys/not-a-uuid/revoke",
            Some(&t),
            None
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &env.app,
            Method::POST,
            &format!("/v1/api-keys/{}/revoke", Uuid::new_v4()),
            Some(&t),
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let weak = Principal {
        permissions: [Permission::TimeRead].into_iter().collect(),
        ..admin
    };
    let r = send(
        &env.app,
        Method::POST,
        &format!("/v1/api-keys/{}/revoke", Uuid::new_v4()),
        Some(&tok(&env, &Caller::Principal(weak))),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    env.db.cleanup().await;
}

// ---------- authenticate ----------

#[tokio::test]
async fn authenticate_resolves_a_key_and_refuses_everything_else_alike() {
    let Some(env) = env().await else { return };
    let (j, admin) = make_firm(&env, "example-attorneys").await;
    let svc = tok(&env, &Caller::Service);
    let good = j["initialApiKey"]["token"].as_str().unwrap().to_owned();

    let r = send(
        &env.app,
        Method::POST,
        "/v1/authenticate",
        Some(&svc),
        Some(json!({"token": good}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let p = r.json();
    assert_eq!(p["tenantId"], j["tenantId"]);
    assert_eq!(p["userId"], j["adminUserId"]);
    assert_eq!(p["keyId"], j["initialApiKey"]["id"]);
    assert_eq!(p["roles"], json!(["admin"]));
    assert!(p["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "keys:manage"));

    // Revoke it through the API, then it is refused at once, with the same body as an unknown key.
    let admin_token = tok(&env, &Caller::Principal(admin.clone()));
    let revoke = format!(
        "/v1/api-keys/{}/revoke",
        j["initialApiKey"]["id"].as_str().unwrap()
    );
    assert_eq!(
        send(&env.app, Method::POST, &revoke, Some(&admin_token), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let revoked = send(
        &env.app,
        Method::POST,
        "/v1/authenticate",
        Some(&svc),
        Some(json!({"token": good}).to_string()),
    )
    .await;
    let unknown = send(
        &env.app,
        Method::POST,
        "/v1/authenticate",
        Some(&svc),
        Some(json!({"token": fake_key()}).to_string()),
    )
    .await;
    let malformed = send(
        &env.app,
        Method::POST,
        "/v1/authenticate",
        Some(&svc),
        Some(json!({"token": "nonsense"}).to_string()),
    )
    .await;
    for r in [&revoked, &unknown, &malformed] {
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(revoked.text, unknown.text);
    assert_eq!(unknown.text, malformed.text);

    // Extra fields are refused.
    let r = send(
        &env.app,
        Method::POST,
        "/v1/authenticate",
        Some(&svc),
        Some(json!({"token": good, "tenant": "x"}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    env.db.cleanup().await;
}

// ---------- routing ----------

#[tokio::test]
async fn unknown_paths_are_a_404_problem_and_wrong_methods_are_refused() {
    let Some(env) = env().await else { return };
    let r = send(&env.app, Method::GET, "/nope", None, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["type"], "urn:timelex:problem:not-found");
    let r = send(
        &env.app,
        Method::GET,
        "/v1/tenants",
        Some(&tok(&env, &Caller::Operator)),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
    env.db.cleanup().await;
}
