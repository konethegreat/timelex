// Written by Kone Tshivhinda & Claude
//! The matters HTTP layer with a real Postgres behind it: authentication, permissions and tenancy.
//! Callers are simulated by signing internal tokens, exactly as the gateway does.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use timelex_auth::{generate_keypair, Caller, Principal, Signer, Verifier};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use tower::ServiceExt;
use uuid::Uuid;

use timelex_matters_service::{http, Store, MIGRATIONS};

struct Env {
    db: TestDb,
    app: Router,
    signer: Signer,
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "matters",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    let keypair = generate_keypair().unwrap();
    let signer = Signer::from_base64("k1", &keypair.private_pkcs8_base64).unwrap();
    let verifier = Verifier::from_config("matters", &format!("k1={}", keypair.public_base64)).unwrap();
    let app = http::router(Store::new(tenants), verifier);
    Some(Env { db, app, signer })
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

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    bearer: Option<&str>,
    content_type: Option<&str>,
    body: Option<String>,
) -> Reply {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(body) => {
            request = request.header(header::CONTENT_TYPE, content_type.unwrap_or("application/json"));
            Body::from(body)
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(request.body(body).unwrap()).await.unwrap();
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

/// A principal as the gateway would present it.
fn principal(tenant: TenantId, permissions: &[Permission]) -> Principal {
    Principal {
        tenant,
        user: UserId::from_uuid(Uuid::new_v4()),
        key: ApiKeyId::from_uuid(Uuid::new_v4()),
        permissions: permissions.iter().copied().collect(),
    }
}

fn token(env: &Env, tenant: TenantId, permissions: &[Permission]) -> String {
    env.signer
        .sign("matters", &Caller::Principal(principal(tenant, permissions)))
        .unwrap()
}

fn reader(env: &Env, tenant: TenantId) -> String {
    token(env, tenant, &[Permission::TimeRead])
}

fn writer(env: &Env, tenant: TenantId) -> String {
    token(env, tenant, &[Permission::SettingsManage])
}

fn member(env: &Env, tenant: TenantId) -> String {
    token(env, tenant, &[Permission::TimeRead, Permission::SettingsManage])
}

async fn create_client(app: &Router, token: &str, client_id: &str, name: &str) -> Reply {
    send(
        app,
        Method::POST,
        "/v1/clients",
        Some(token),
        None,
        Some(json!({ "clientId": client_id, "name": name }).to_string()),
    )
    .await
}

async fn create_matter(app: &Router, token: &str, body: Value) -> Reply {
    send(
        app,
        Method::POST,
        "/v1/matters",
        Some(token),
        None,
        Some(body.to_string()),
    )
    .await
}

// ---------- health and authentication ----------

#[tokio::test]
async fn health_needs_no_credentials() {
    let Some(env) = env().await else { return };
    let reply = send(&env.app, Method::GET, "/health", None, None, None).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["status"], "ok");
    env.db.cleanup().await;
}

#[tokio::test]
async fn routes_need_a_principal_token() {
    let Some(env) = env().await else { return };
    for path in ["/v1/clients", "/v1/matters", "/v1/aliases", "/v1/rate-cards"] {
        let reply = send(&env.app, Method::GET, path, None, None, None).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(reply.json()["type"], "urn:timelex:problem:unauthenticated");
    }

    let operator = env.signer.sign("matters", &Caller::Operator).unwrap();
    let reply = send(&env.app, Method::GET, "/v1/clients", Some(&operator), None, None).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    env.db.cleanup().await;
}

#[tokio::test]
async fn reads_need_time_read_and_writes_need_settings_manage() {
    let Some(env) = env().await else { return };
    let firm = tenant();
    let read_only = reader(&env, firm);

    let reply = send(&env.app, Method::GET, "/v1/clients", Some(&read_only), None, None).await;
    assert_eq!(reply.status, StatusCode::OK);

    let reply = create_client(&env.app, &read_only, "cl_1", "Meridian Logistics").await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    let write_only = writer(&env, firm);
    let reply = send(
        &env.app,
        Method::GET,
        "/v1/clients",
        Some(&write_only),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);

    let reply = create_client(&env.app, &write_only, "cl_1", "Meridian Logistics").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    env.db.cleanup().await;
}

// ---------- clients, matters and tenancy ----------

#[tokio::test]
async fn a_firm_sees_only_its_own_clients_and_matters() {
    let Some(env) = env().await else { return };
    let firm_a = tenant();
    let firm_b = tenant();
    let a = member(&env, firm_a);
    let b = member(&env, firm_b);

    let reply = create_client(&env.app, &a, "cl_meridian", "Meridian Logistics").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json()["clientId"], "cl_meridian");
    assert_eq!(reply.json()["status"], "active");

    let reply = create_matter(
        &env.app,
        &a,
        json!({
            "matterId": "mt_lease",
            "clientId": "cl_meridian",
            "reference": "M-2026-0102",
            "title": "Commercial lease renewal",
            "practiceArea": "Litigation",
            "openedOn": "2026-01-15"
        }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json()["openedOn"], "2026-01-15");
    assert_eq!(reply.json()["status"], "open");

    let listed = send(&env.app, Method::GET, "/v1/clients", Some(&a), None, None).await;
    assert_eq!(listed.json()["clients"].as_array().unwrap().len(), 1);

    let other = send(&env.app, Method::GET, "/v1/clients", Some(&b), None, None).await;
    assert_eq!(other.status, StatusCode::OK);
    assert_eq!(
        other.json()["clients"].as_array().unwrap().len(),
        0,
        "firm B saw firm A's client"
    );
    let other = send(&env.app, Method::GET, "/v1/matters", Some(&b), None, None).await;
    assert_eq!(other.json()["matters"].as_array().unwrap().len(), 0);

    // A row cannot reference another firm's client.
    let reply = create_matter(
        &env.app,
        &b,
        json!({
            "matterId": "mt_foreign",
            "clientId": "cl_meridian",
            "reference": "M-2026-9000",
            "title": "Foreign matter"
        }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    env.db.cleanup().await;
}

#[tokio::test]
async fn matters_can_be_narrowed_to_one_client() {
    let Some(env) = env().await else { return };
    let firm = tenant();
    let token = member(&env, firm);
    create_client(&env.app, &token, "cl_a", "Client A").await;
    create_client(&env.app, &token, "cl_b", "Client B").await;
    for (matter, client, reference) in [
        ("mt_1", "cl_a", "M-2026-0001"),
        ("mt_2", "cl_a", "M-2026-0002"),
        ("mt_3", "cl_b", "M-2026-0003"),
    ] {
        let reply = create_matter(
            &env.app,
            &token,
            json!({"matterId": matter, "clientId": client, "reference": reference, "title": "Work"}),
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    }

    let reply = send(
        &env.app,
        Method::GET,
        "/v1/matters?clientId=cl_a",
        Some(&token),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let matters = reply.json();
    let matters = matters["matters"].as_array().unwrap();
    assert_eq!(matters.len(), 2);
    assert!(matters.iter().all(|matter| matter["clientId"] == "cl_a"));

    let reply = send(
        &env.app,
        Method::GET,
        "/v1/matters?clientId=%20",
        Some(&token),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    env.db.cleanup().await;
}

#[tokio::test]
async fn duplicate_identifiers_and_references_are_conflicts() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    create_client(&env.app, &token, "cl_a", "Client A").await;
    create_client(&env.app, &token, "cl_b", "Client B").await;

    let reply = create_client(&env.app, &token, "cl_a", "Someone else").await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.json()["type"], "urn:timelex:problem:conflict");

    let body = json!({
        "matterId": "mt_1",
        "clientId": "cl_a",
        "reference": "M-2026-0001",
        "title": "First"
    });
    let reply = create_matter(&env.app, &token, body.clone()).await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);

    let reply = create_matter(&env.app, &token, body).await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    let reply = create_matter(
        &env.app,
        &token,
        json!({
            "matterId": "mt_2",
            "clientId": "cl_b",
            "reference": "m-2026-0001 ",
            "title": "Second"
        }),
    )
    .await;
    assert_eq!(
        reply.status,
        StatusCode::CONFLICT,
        "reference_keys are case- and space-insensitive"
    );
    env.db.cleanup().await;
}

// ---------- aliases ----------

#[tokio::test]
async fn aliases_are_idempotent_per_target_and_conflict_across_targets() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    create_client(&env.app, &token, "cl_a", "Client A").await;
    create_client(&env.app, &token, "cl_b", "Client B").await;

    let body = json!({"kind": "client_domain", "value": "Meridian.Example", "clientId": "cl_a"});
    let reply = send(
        &env.app,
        Method::POST,
        "/v1/aliases",
        Some(&token),
        None,
        Some(body.to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json()["value"], "meridian.example");
    let id = reply.json()["id"].clone();

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/aliases",
        Some(&token),
        None,
        Some(body.to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "same target is idempotent");
    assert_eq!(reply.json()["id"], id);

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/aliases",
        Some(&token),
        None,
        Some(json!({"kind": "client_domain", "value": "meridian.example", "clientId": "cl_b"}).to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    let reply = send(
        &env.app,
        Method::GET,
        "/v1/aliases?kind=client_domain&value=meridian.example",
        Some(&token),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["aliases"].as_array().unwrap().len(), 1);

    let reply = send(
        &env.app,
        Method::GET,
        "/v1/aliases?value=meridian.example",
        Some(&token),
        None,
        None,
    )
    .await;
    assert_eq!(
        reply.status,
        StatusCode::BAD_REQUEST,
        "a value without a kind cannot be canonicalised"
    );
    env.db.cleanup().await;
}

// ---------- rate cards ----------

#[tokio::test]
async fn a_rate_card_is_replaced_and_listed() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    create_client(&env.app, &token, "cl_a", "Client A").await;
    create_matter(
        &env.app,
        &token,
        json!({
            "matterId": "mt_1",
            "clientId": "cl_a",
            "reference": "M-2026-0001",
            "title": "Work"
        }),
    )
    .await;

    let reply = send(
        &env.app,
        Method::PUT,
        "/v1/matters/mt_1/rate-card",
        Some(&token),
        None,
        Some(json!({"hourlyRateCents": 280000}).to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json()["hourlyRateCents"], 280000);
    assert_eq!(reply.json()["unitMinutes"], 6);
    assert_eq!(reply.json()["rounding"], "up");
    assert_eq!(reply.json()["minimumUnits"], 1);

    let reply = send(
        &env.app,
        Method::PUT,
        "/v1/matters/mt_1/rate-card",
        Some(&token),
        None,
        Some(
            json!({
                "hourlyRateCents": 195000,
                "unitMinutes": 15,
                "rounding": "nearest",
                "minimumUnits": 0
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["hourlyRateCents"], 195000);

    let reply = send(&env.app, Method::GET, "/v1/rate-cards", Some(&token), None, None).await;
    let cards = reply.json();
    let cards = cards["rateCards"].as_array().unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["unitMinutes"], 15);
    assert_eq!(cards[0]["rounding"], "nearest");

    let reply = send(
        &env.app,
        Method::PUT,
        "/v1/matters/mt_missing/rate-card",
        Some(&token),
        None,
        Some(json!({"hourlyRateCents": 1000}).to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    env.db.cleanup().await;
}

// ---------- refusals ----------

#[tokio::test]
async fn bad_bodies_are_refused_without_echoing_them() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    create_client(&env.app, &token, "cl_a", "Client A").await;

    for body in [
        json!({"clientId": "has space", "name": "Bad"}),
        json!({"clientId": "cl_x", "name": "  "}),
        json!({"clientId": "cl_x", "name": "Fine", "tenantId": Uuid::new_v4()}),
        json!({"clientId": "cl_x"}),
    ] {
        let reply = send(
            &env.app,
            Method::POST,
            "/v1/clients",
            Some(&token),
            None,
            Some(body.to_string()),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(reply.json()["type"], "urn:timelex:problem:invalid-request");
    }

    for body in [
        json!({
            "matterId": "mt_x",
            "clientId": "cl_a",
            "reference": "M-2026-0001",
            "title": "Dates",
            "openedOn": "2026-05-02",
            "closedOn": "2026-05-01"
        }),
        json!({
            "matterId": "mt_x",
            "clientId": "cl_a",
            "reference": "M-2026-0001",
            "title": "Status",
            "status": "archived"
        }),
        json!({
            "matterId": "mt_x",
            "clientId": "cl_a",
            "reference": "M-2026-0001",
            "title": "Date",
            "openedOn": "2026-02-30"
        }),
    ] {
        let reply = create_matter(&env.app, &token, body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
    }

    let reply = send(
        &env.app,
        Method::PUT,
        "/v1/matters/mt_x/rate-card",
        Some(&token),
        None,
        Some(json!({"hourlyRateCents": 100000001}).to_string()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    env.db.cleanup().await;
}

#[tokio::test]
async fn imports_refuse_a_wrong_media_type_and_unknown_kind() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/import/clients",
        Some(&token),
        Some("application/json"),
        Some(r#"{"clientId":"cl_1"}"#.into()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/import/contacts",
        Some(&token),
        Some("text/csv"),
        Some("clientId,name\ncl_1,A\n".into()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/import/clients",
        Some(&token),
        Some("text/csv; charset=utf-8"),
        Some("clientId,name,tenantId\ncl_1,A,b\n".into()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.json()["detail"]
        .as_str()
        .unwrap()
        .contains("unknown column"));

    let reply = send(
        &env.app,
        Method::POST,
        "/v1/import/clients",
        Some(&token),
        Some("text/csv"),
        Some("clientId,name\ncl_1,A\n".into()),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json()["kind"], "clients");
    assert_eq!(reply.json()["rows"], 1);
    env.db.cleanup().await;
}

#[tokio::test]
async fn unknown_paths_are_a_404_problem() {
    let Some(env) = env().await else { return };
    let reply = send(&env.app, Method::GET, "/nope", None, None, None).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.json()["type"], "urn:timelex:problem:not-found");
    env.db.cleanup().await;
}
