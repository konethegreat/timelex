// Written by Kone Tshivhinda & Claude
//! CSV import against a real Postgres: all-or-nothing application, idempotent upserts, and the
//! gold-set firm (8 clients, 24 matters) that P3-05 will attribute against.

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

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4())
}

fn member(env: &Env, tenant: TenantId) -> String {
    let principal = Principal {
        tenant,
        user: UserId::from_uuid(Uuid::new_v4()),
        key: ApiKeyId::from_uuid(Uuid::new_v4()),
        permissions: [Permission::TimeRead, Permission::SettingsManage]
            .into_iter()
            .collect(),
    };
    env.signer.sign("matters", &Caller::Principal(principal)).unwrap()
}

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    token: &str,
    content_type: Option<&str>,
    body: Option<String>,
) -> (StatusCode, String) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let body = match body {
        Some(body) => {
            builder = builder.header(header::CONTENT_TYPE, content_type.unwrap_or("application/json"));
            Body::from(body)
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(builder.body(body).unwrap()).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    (parts.status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn import(app: &Router, token: &str, kind: &str, csv: &str) -> (StatusCode, String) {
    request(
        app,
        Method::POST,
        &format!("/v1/import/{kind}"),
        token,
        Some("text/csv"),
        Some(csv.into()),
    )
    .await
}

async fn get(app: &Router, token: &str, path: &str) -> Value {
    let (status, text) = request(app, Method::GET, path, token, None, None).await;
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    serde_json::from_str(&text).unwrap()
}

fn csv(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer.write_record(headers).unwrap();
    for row in rows {
        writer.write_record(row).unwrap();
    }
    String::from_utf8(writer.into_inner().unwrap()).unwrap()
}

fn firm_csv() -> String {
    csv(
        &["clientId", "name"],
        vec![
            vec!["cl_a".into(), "Meridian Logistics".into()],
            vec!["cl_b".into(), "Kalahari Mining".into()],
        ],
    )
}

fn matters_csv() -> String {
    csv(
        &[
            "matterId",
            "clientId",
            "reference",
            "title",
            "status",
            "practiceArea",
            "openedOn",
        ],
        vec![
            vec![
                "mt_1".into(),
                "cl_a".into(),
                "M-2026-0101".into(),
                "Shareholder agreement review".into(),
                "open".into(),
                "Corporate".into(),
                "2026-01-10".into(),
            ],
            vec![
                "mt_2".into(),
                "cl_a".into(),
                "M-2026-0102".into(),
                "Commercial lease renewal".into(),
                "open".into(),
                "Litigation".into(),
                String::new(),
            ],
            vec![
                "mt_3".into(),
                "cl_b".into(),
                "M-2026-0103".into(),
                "Mining right transfer".into(),
                "closed".into(),
                "Property".into(),
                "2025-11-03".into(),
            ],
        ],
    )
}

fn aliases_csv() -> String {
    csv(
        &["kind", "value", "clientId", "matterId"],
        vec![
            vec![
                "client_domain".into(),
                "Meridian.Example".into(),
                "cl_a".into(),
                String::new(),
            ],
            vec![
                "client_domain".into(),
                "kalahari.example".into(),
                "cl_b".into(),
                String::new(),
            ],
            vec![
                "client_name".into(),
                "Meridian Logistics".into(),
                "cl_a".into(),
                String::new(),
            ],
            vec![
                "matter_reference".into(),
                "M-2026-0101".into(),
                "cl_a".into(),
                "mt_1".into(),
            ],
        ],
    )
}

fn rate_cards_csv() -> String {
    csv(
        &[
            "matterId",
            "hourlyRateCents",
            "unitMinutes",
            "rounding",
            "minimumUnits",
        ],
        vec![
            vec![
                "mt_1".into(),
                "280000".into(),
                String::new(),
                String::new(),
                String::new(),
            ],
            vec![
                "mt_2".into(),
                "195000".into(),
                "15".into(),
                "nearest".into(),
                "0".into(),
            ],
        ],
    )
}

fn count(value: &Value, key: &str) -> usize {
    value[key].as_array().unwrap().len()
}

#[tokio::test]
async fn a_bad_row_rolls_back_the_whole_import() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    let body = csv(
        &["clientId", "name"],
        vec![
            vec!["cl_good".into(), "Good Client".into()],
            vec!["bad id!".into(), "Bad Client".into()],
        ],
    );

    let (status, text) = import(&env.app, &token, "clients", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    let problem: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(problem["type"], "urn:timelex:problem:invalid-request");
    assert!(
        problem["detail"].as_str().unwrap().starts_with("row 3:"),
        "{}",
        problem["detail"]
    );

    assert_eq!(
        count(&get(&env.app, &token, "/v1/clients").await, "clients"),
        0,
        "the first row must have been rolled back"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn imports_apply_in_order_and_are_idempotent() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());

    for (kind, body, rows) in [
        ("clients", firm_csv(), 2),
        ("matters", matters_csv(), 3),
        ("aliases", aliases_csv(), 4),
        ("rate-cards", rate_cards_csv(), 2),
    ] {
        let (status, text) = import(&env.app, &token, kind, &body).await;
        assert_eq!(status, StatusCode::OK, "{kind}: {text}");
        let reply: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(reply["kind"], kind);
        assert_eq!(reply["rows"], rows, "{kind}");
    }

    assert_eq!(count(&get(&env.app, &token, "/v1/clients").await, "clients"), 2);
    assert_eq!(count(&get(&env.app, &token, "/v1/matters").await, "matters"), 3);
    assert_eq!(count(&get(&env.app, &token, "/v1/aliases").await, "aliases"), 4);
    assert_eq!(
        count(&get(&env.app, &token, "/v1/rate-cards").await, "rateCards"),
        2
    );

    let aliases = get(
        &env.app,
        &token,
        "/v1/aliases?kind=client_domain&value=meridian.example",
    )
    .await;
    let alias = &aliases["aliases"][0];
    assert_eq!(alias["clientId"], "cl_a");
    assert_eq!(alias["value"], "meridian.example");

    let cards = get(&env.app, &token, "/v1/rate-cards").await;
    let card = cards["rateCards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["matterId"] == "mt_2")
        .unwrap();
    assert_eq!(card["unitMinutes"], 15);
    assert_eq!(card["rounding"], "nearest");
    assert_eq!(card["minimumUnits"], 0);

    // The same file again updates rather than duplicates.
    let renamed = csv(
        &["clientId", "name"],
        vec![
            vec!["cl_a".into(), "Meridian Logistics Ltd".into()],
            vec!["cl_b".into(), "Kalahari Mining".into()],
        ],
    );
    let (status, _) = import(&env.app, &token, "clients", &renamed).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = import(&env.app, &token, "aliases", &aliases_csv()).await;
    assert_eq!(status, StatusCode::OK);

    let clients = get(&env.app, &token, "/v1/clients").await;
    assert_eq!(count(&clients, "clients"), 2);
    assert_eq!(clients["clients"][0]["name"], "Meridian Logistics Ltd");
    assert_eq!(
        count(&get(&env.app, &token, "/v1/aliases").await, "aliases"),
        4,
        "re-importing aliases must not duplicate them"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn matters_with_one_reference_cannot_arrive_as_two_matters() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    let (status, _) = import(&env.app, &token, "clients", &firm_csv()).await;
    assert_eq!(status, StatusCode::OK);

    let body = csv(
        &["matterId", "clientId", "reference", "title"],
        vec![
            vec!["mt_1".into(), "cl_a".into(), "M-2026-0101".into(), "First".into()],
            vec![
                "mt_2".into(),
                "cl_b".into(),
                "M-2026-0101".into(),
                "Second".into(),
            ],
        ],
    );
    let (status, text) = import(&env.app, &token, "matters", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    let problem: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        problem["detail"],
        "row 3: a matter with this reference already exists"
    );
    assert_eq!(
        count(&get(&env.app, &token, "/v1/matters").await, "matters"),
        0,
        "the first matter must have been rolled back"
    );
    env.db.cleanup().await;
}

#[tokio::test]
async fn the_gold_set_firm_imports_completely() {
    let Some(env) = env().await else { return };
    let token = member(&env, tenant());
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/synthetic/corpus");
    let firm: Value =
        serde_json::from_str(&std::fs::read_to_string(corpus.join("firm.json")).unwrap()).unwrap();
    let labels: Value =
        serde_json::from_str(&std::fs::read_to_string(corpus.join("labels.json")).unwrap()).unwrap();

    let clients = firm["clients"].as_array().unwrap();
    let matters = firm["matters"].as_array().unwrap();

    let client_csv = csv(
        &["clientId", "name"],
        clients
            .iter()
            .map(|client| {
                vec![
                    client["clientId"].as_str().unwrap().to_owned(),
                    client["name"].as_str().unwrap().to_owned(),
                ]
            })
            .collect(),
    );
    let matter_csv = csv(
        &[
            "matterId",
            "clientId",
            "reference",
            "title",
            "status",
            "practiceArea",
        ],
        matters
            .iter()
            .map(|matter| {
                vec![
                    matter["matterId"].as_str().unwrap().to_owned(),
                    matter["clientId"].as_str().unwrap().to_owned(),
                    matter["reference"].as_str().unwrap().to_owned(),
                    matter["title"].as_str().unwrap().to_owned(),
                    matter["status"].as_str().unwrap().to_owned(),
                    matter["practiceArea"].as_str().unwrap().to_owned(),
                ]
            })
            .collect(),
    );
    let mut alias_rows = Vec::new();
    for client in clients {
        alias_rows.push(vec![
            "client_domain".to_owned(),
            client["domain"].as_str().unwrap().to_owned(),
            client["clientId"].as_str().unwrap().to_owned(),
            String::new(),
        ]);
    }
    for matter in matters {
        alias_rows.push(vec![
            "matter_reference".to_owned(),
            matter["reference"].as_str().unwrap().to_owned(),
            matter["clientId"].as_str().unwrap().to_owned(),
            matter["matterId"].as_str().unwrap().to_owned(),
        ]);
    }
    let alias_csv = csv(&["kind", "value", "clientId", "matterId"], alias_rows);
    let card_csv = csv(
        &["matterId", "hourlyRateCents"],
        matters
            .iter()
            .map(|matter| {
                vec![
                    matter["matterId"].as_str().unwrap().to_owned(),
                    matter["rateCentsPerHour"].as_i64().unwrap().to_string(),
                ]
            })
            .collect(),
    );

    for (kind, body, rows) in [
        ("clients", &client_csv, clients.len()),
        ("matters", &matter_csv, matters.len()),
        ("aliases", &alias_csv, clients.len() + matters.len()),
        ("rate-cards", &card_csv, matters.len()),
    ] {
        let (status, text) = import(&env.app, &token, kind, body).await;
        assert_eq!(status, StatusCode::OK, "{kind}: {text}");
        let reply: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(reply["rows"], rows, "{kind}");
    }

    let imported_clients = get(&env.app, &token, "/v1/clients").await;
    assert_eq!(count(&imported_clients, "clients"), 8);
    let imported_matters = get(&env.app, &token, "/v1/matters").await;
    assert_eq!(count(&imported_matters, "matters"), 24);
    let closed = imported_matters["matters"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|matter| matter["status"] == "closed")
        .count();
    assert_eq!(closed, 2);
    assert_eq!(
        count(&get(&env.app, &token, "/v1/rate-cards").await, "rateCards"),
        24
    );
    assert_eq!(count(&get(&env.app, &token, "/v1/aliases").await, "aliases"), 32);

    let found = get(
        &env.app,
        &token,
        "/v1/aliases?kind=matter_reference&value=M-2026-0101",
    )
    .await;
    assert_eq!(found["aliases"][0]["matterId"], "mt_f5909ad33820d983");
    assert_eq!(found["aliases"][0]["clientId"], "cl_92c352e5b3f9b6fc");

    let cards = get(&env.app, &token, "/v1/rate-cards").await;
    let card = cards["rateCards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["matterId"] == "mt_f5909ad33820d983")
        .unwrap();
    assert_eq!(card["hourlyRateCents"], 280000);

    // Every matter a label points at is importable, which is the precondition for P3-05.
    let imported: std::collections::BTreeSet<&str> = imported_matters["matters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|matter| matter["matterId"].as_str().unwrap())
        .collect();
    for label in labels["labels"].as_array().unwrap() {
        let mut wanted = Vec::new();
        if let Some(matter) = label["matterId"].as_str() {
            wanted.push(matter);
        }
        for candidate in label["candidateMatterIds"].as_array().unwrap() {
            wanted.push(candidate.as_str().unwrap());
        }
        for matter in wanted {
            assert!(imported.contains(matter), "label references unknown {matter}");
        }
    }
    env.db.cleanup().await;
}

#[tokio::test]
async fn an_import_cannot_see_another_firms_rows() {
    let Some(env) = env().await else { return };
    let firm_a = member(&env, tenant());
    let firm_b = member(&env, tenant());

    let (status, _) = import(&env.app, &firm_a, "clients", &firm_csv()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count(&get(&env.app, &firm_b, "/v1/clients").await, "clients"), 0);

    // Firm B's matters cannot attach to firm A's client.
    let (status, text) = import(&env.app, &firm_b, "matters", &matters_csv()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    assert_eq!(
        json!(0),
        json!(count(&get(&env.app, &firm_b, "/v1/matters").await, "matters"))
    );
    env.db.cleanup().await;
}
