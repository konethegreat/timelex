// Written by Kone Tshivhinda & Claude
//! The gateway's authentication and routing against a *stub* identity service that records what it
//! receives. No database: these tests are about what the gateway forwards, refuses and hides.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::response::Response;
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use timelex_auth::{generate_keypair, Caller, Verifier};
use timelex_gateway::config::{sha256, Config};
use tower::ServiceExt;
use uuid::Uuid;

/// Random per run: no credential-shaped literal in the source, and no two runs share one.
static OPERATOR_KEY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("operator-{}", Uuid::new_v4().simple()));
/// A well-formed but unknown tenant key, built at run time so no key-shaped literal sits in the source
/// (secret scanners flag those, and a real key never belongs in a repository).
static TENANT_KEY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let tenant = timelex_identity::TenantId::from_uuid(Uuid::from_u128(0x0191a2b3c4d57e6f8a9b0c1d2e3f4a5b));
    timelex_identity::ApiKeyToken::new(tenant, [7u8; 32])
        .expose_secret()
        .to_owned()
});

#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

struct Stub {
    addr: std::net::SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Stub {
    fn calls(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

type Behaviour = dyn Fn(&Seen) -> (StatusCode, String, Duration) + Send + Sync;

/// A fake identity service: records every request, answers as `behaviour` says.
async fn stub(behaviour: impl Fn(&Seen) -> (StatusCode, String, Duration) + Send + Sync + 'static) -> Stub {
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let behaviour: Arc<Behaviour> = Arc::new(behaviour);
    let log = seen.clone();
    let app = Router::new().fallback(move |req: Request| {
        let (log, behaviour) = (log.clone(), behaviour.clone());
        async move {
            let (parts, body) = req.into_parts();
            let bytes = body.collect().await.unwrap().to_bytes();
            let s = Seen {
                method: parts.method.to_string(),
                path: parts
                    .uri
                    .path_and_query()
                    .map(|p| p.to_string())
                    .unwrap_or_default(),
                headers: parts
                    .headers
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_owned()))
                    .collect(),
                body: String::from_utf8_lossy(&bytes).into_owned(),
            };
            log.lock().unwrap().push(s.clone());
            let (status, text, delay) = behaviour(&s);
            tokio::time::sleep(delay).await;
            let mut r = Response::new(Body::from(text));
            *r.status_mut() = status;
            r.headers_mut()
                .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
            r
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Stub { addr, seen }
}

fn principal_reply() -> String {
    json!({"tenantId": Uuid::from_u128(11), "userId": Uuid::from_u128(22), "keyId": Uuid::from_u128(33),
           "roles": ["admin"], "permissions": ["keys:manage", "time:read"]})
    .to_string()
}

/// Identity as it behaves when everything is fine.
fn happy(s: &Seen) -> (StatusCode, String, Duration) {
    match s.path.as_str() {
        "/v1/authenticate" => (StatusCode::OK, principal_reply(), Duration::ZERO),
        _ => (
            StatusCode::CREATED,
            json!({"ok": true}).to_string(),
            Duration::ZERO,
        ),
    }
}

struct Gw {
    app: Router,
    verifier: Verifier,
}

fn gateway(identity_url: String, timeout: Duration) -> Gw {
    let kp = generate_keypair().unwrap();
    let config = Config {
        signer: timelex_auth::Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
        operator_key_hashes: vec![sha256(OPERATOR_KEY.as_str())],
        identity_url,
        upstream_timeout: timeout,
    };
    Gw {
        app: timelex_gateway::app_with(config).unwrap(),
        verifier: Verifier::from_config("identity", &format!("k1={}", kp.public_base64)).unwrap(),
    }
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    text: String,
}

async fn call(app: &Router, method: Method, path: &str, extra: &[(&str, &str)], body: Option<&str>) -> Reply {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_owned())
        }
        None => Body::empty(),
    };
    let r = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let (parts, body) = r.into_parts();
    Reply {
        status: parts.status,
        headers: parts.headers,
        text: String::from_utf8_lossy(&body.collect().await.unwrap().to_bytes()).into_owned(),
    }
}

fn bearer(token: &str) -> [(&'static str, String); 1] {
    [("authorization", format!("Bearer {token}"))]
}

async fn post_as(app: &Router, path: &str, token: &str, body: Option<&str>) -> Reply {
    let b = bearer(token);
    call(
        app,
        Method::POST,
        path,
        &[("authorization", b[0].1.as_str())],
        body,
    )
    .await
}

// ---------- open routes ----------

#[tokio::test]
async fn health_and_meta_stay_open_and_unknown_paths_are_a_404_problem() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    assert_eq!(
        call(&gw.app, Method::GET, "/health", &[], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        call(&gw.app, Method::GET, "/v1/meta", &[], None).await.status,
        StatusCode::OK
    );
    let r = call(&gw.app, Method::GET, "/nope", &[], None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(r.text.contains("urn:timelex:problem:not-found"));
    assert!(s.calls().is_empty(), "open routes must not touch identity");
}

#[tokio::test]
async fn authenticate_is_not_reachable_from_outside() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    for token in [OPERATOR_KEY.as_str(), TENANT_KEY.as_str()] {
        let r = post_as(
            &gw.app,
            "/v1/authenticate",
            token,
            Some(&json!({"token": TENANT_KEY.as_str()}).to_string()),
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
    }
    assert!(s.calls().is_empty());
}

// ---------- tenant routes: what identity receives ----------

#[tokio::test]
async fn a_tenant_call_is_authenticated_then_forwarded_with_a_principal_token() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let body = json!({"name": "k", "scopes": ["time:read"]}).to_string();
    let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&body)).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(
        r.text,
        json!({"ok": true}).to_string(),
        "the service's answer is relayed"
    );

    let calls = s.calls();
    assert_eq!(calls.len(), 2, "one authenticate, one forward");

    // 1. authenticate: a *service* token, and the API key only in the body.
    let auth = &calls[0];
    assert_eq!(
        (auth.method.as_str(), auth.path.as_str()),
        ("POST", "/v1/authenticate")
    );
    let who = gw
        .verifier
        .verify(
            auth.header("authorization")
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(who.caller, Caller::Service);
    assert_eq!(
        serde_json::from_str::<Value>(&auth.body).unwrap(),
        json!({"token": TENANT_KEY.as_str()})
    );

    // 2. the forwarded call: a *principal* token built from identity's answer.
    let fwd = &calls[1];
    assert_eq!((fwd.method.as_str(), fwd.path.as_str()), ("POST", "/v1/api-keys"));
    assert_eq!(fwd.body, body);
    let verified = gw
        .verifier
        .verify(
            fwd.header("authorization")
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap(),
        )
        .unwrap();
    match verified.caller {
        Caller::Principal(p) => {
            assert_eq!(p.tenant.as_uuid(), Uuid::from_u128(11));
            assert_eq!(p.user.as_uuid(), Uuid::from_u128(22));
            assert_eq!(p.key.as_uuid(), Uuid::from_u128(33));
            let perms: Vec<&str> = p.permissions.iter().map(|p| p.as_str()).collect();
            assert_eq!(perms, ["keys:manage", "time:read"]);
        }
        other => panic!("expected a principal, got {other:?}"),
    }
    // The URL never carries the key.
    assert!(!fwd.path.contains("tlx_"));
}

#[tokio::test]
async fn nothing_a_client_chose_reaches_the_service_except_the_body() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let b = bearer(TENANT_KEY.as_str());
    let extra = [
        ("authorization", b[0].1.as_str()),
        ("x-timelex-tenant", "someone-elses-firm"),
        ("x-timelex-user", "admin"),
        ("x-forwarded-for", "10.0.0.1"),
        ("cookie", "session=abc"),
        ("idempotency-key", "k-1"),
        (
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        ),
    ];
    let r = call(
        &gw.app,
        Method::POST,
        "/v1/api-keys?tenantId=other&user=admin",
        &extra,
        Some(&json!({"name":"k","scopes":["time:read"]}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    for seen in s.calls() {
        // An allowlist, not a blocklist: the service may receive exactly these headers and no others,
        // so a header added to the forwarding code by mistake fails here whatever it is called.
        let allowed = [
            "host",
            "authorization",
            "content-type",
            "content-length",
            "accept",
        ];
        for (name, value) in &seen.headers {
            assert!(
                allowed.contains(&name.as_str()),
                "unexpected header reached the service: {name}"
            );
            assert!(!value.contains("tlx_"), "an API key appeared in header {name}");
        }
        for forbidden in [
            "x-timelex-tenant",
            "x-timelex-user",
            "x-forwarded-for",
            "cookie",
            "idempotency-key",
            "traceparent",
        ] {
            assert!(
                seen.header(forbidden).is_none(),
                "{forbidden} reached the service"
            );
        }
        assert!(
            !seen.path.contains('?'),
            "the query string was forwarded: {}",
            seen.path
        );
        let auth = seen.header("authorization").unwrap();
        assert!(
            !auth.contains("tlx_") && !auth.contains(TENANT_KEY.as_str()),
            "the API key reached the service"
        );
    }
}

#[tokio::test]
async fn revoke_forwards_a_canonical_path_and_refuses_bad_ids_after_authenticating() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let id = Uuid::new_v4();
    let upper = id.to_string().to_uppercase();
    let r = post_as(
        &gw.app,
        &format!("/v1/api-keys/{upper}/revoke"),
        TENANT_KEY.as_str(),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(
        s.calls().last().unwrap().path,
        format!("/v1/api-keys/{id}/revoke"),
        "re-formatted canonically"
    );

    let before = s.calls().len();
    let r = post_as(
        &gw.app,
        "/v1/api-keys/..%2f..%2fv1%2ftenants/revoke",
        TENANT_KEY.as_str(),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        s.calls().len(),
        before + 1,
        "only the authenticate call was made; nothing forwarded"
    );

    // An unauthenticated caller sending a bad id still just gets the 401.
    let r = call(&gw.app, Method::POST, "/v1/api-keys/not-a-uuid/revoke", &[], None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

// ---------- operator route ----------

#[tokio::test]
async fn only_the_configured_operator_key_may_create_firms() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let body = json!({"slug": "example-attorneys"}).to_string();

    let r = post_as(&gw.app, "/v1/tenants", OPERATOR_KEY.as_str(), Some(&body)).await;
    assert_eq!(r.status, StatusCode::CREATED);
    let calls = s.calls();
    assert_eq!(calls.len(), 1, "operator calls never go through authenticate");
    let verified = gw
        .verifier
        .verify(
            calls[0]
                .header("authorization")
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(verified.caller, Caller::Operator);

    let padded = format!("{} ", OPERATOR_KEY.as_str());
    for wrong in ["wrong", TENANT_KEY.as_str(), "", padded.as_str()] {
        let r = if wrong.is_empty() {
            call(&gw.app, Method::POST, "/v1/tenants", &[], Some(&body)).await
        } else {
            post_as(&gw.app, "/v1/tenants", wrong, Some(&body)).await
        };
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{wrong:?}");
    }
    assert_eq!(s.calls().len(), 1, "no refused call reached identity");
}

#[tokio::test]
async fn operator_key_rotation_accepts_both_keys() {
    let s = stub(happy).await;
    let kp = generate_keypair().unwrap();
    let config = Config {
        signer: timelex_auth::Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
        operator_key_hashes: vec![sha256("old-key"), sha256("new-key")],
        identity_url: format!("http://{}", s.addr),
        upstream_timeout: Duration::from_secs(2),
    };
    let app = timelex_gateway::app_with(config).unwrap();
    for key in ["old-key", "new-key"] {
        assert_eq!(
            post_as(&app, "/v1/tenants", key, Some("{}")).await.status,
            StatusCode::CREATED,
            "{key}"
        );
    }
    assert_eq!(
        post_as(&app, "/v1/tenants", "third-key", Some("{}")).await.status,
        StatusCode::UNAUTHORIZED
    );
}

// ---------- uniform 401 ----------

#[tokio::test]
async fn every_unusable_credential_gets_the_same_401_and_never_reaches_a_service_route() {
    // Identity says "unknown key" to authenticate.
    let s = stub(|s: &Seen| match s.path.as_str() {
        "/v1/authenticate" => (
            StatusCode::UNAUTHORIZED,
            json!({"type":"urn:timelex:problem:unauthenticated"}).to_string(),
            Duration::ZERO,
        ),
        _ => (StatusCode::CREATED, "{}".into(), Duration::ZERO),
    })
    .await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let body = json!({"name":"k","scopes":["time:read"]}).to_string();
    let unknown_key_header = format!("Bearer {}", TENANT_KEY.as_str());
    let attempts: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("no header", vec![]),
        (
            "another scheme",
            vec![("authorization", "Digest username=nobody")],
        ),
        ("empty bearer", vec![("authorization", "Bearer ")]),
        (
            "two headers",
            vec![("authorization", "Bearer a"), ("authorization", "Bearer b")],
        ),
        (
            "unknown key",
            vec![("authorization", unknown_key_header.as_str())],
        ),
        ("garbage", vec![("authorization", "Bearer nonsense")]),
    ];
    let mut bodies = Vec::new();
    for (label, headers) in attempts {
        let r = call(&gw.app, Method::POST, "/v1/api-keys", &headers, Some(&body)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{label}");
        assert_eq!(r.headers[header::WWW_AUTHENTICATE], "Bearer", "{label}");
        bodies.push(r.text);
    }
    assert!(
        bodies.windows(2).all(|w| w[0] == w[1]),
        "401 bodies differ: {bodies:?}"
    );
    assert!(
        s.calls().iter().all(|c| c.path == "/v1/authenticate"),
        "a refused call was forwarded"
    );
}

// ---------- failures of the service behind ----------

#[tokio::test]
async fn an_unreachable_identity_is_a_502_and_nothing_is_forwarded() {
    // A server that accepts the connection and drops it at once. (A closed port is not used: on Windows
    // it times out instead of refusing, which would make this test depend on the platform.)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
    let gw = gateway(format!("http://{addr}"), Duration::from_secs(2));
    let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}")).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    let r = post_as(&gw.app, "/v1/tenants", OPERATOR_KEY.as_str(), Some("{}")).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn a_slow_identity_is_a_504() {
    let s = stub(|_| (StatusCode::OK, principal_reply(), Duration::from_millis(1500))).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_millis(300));
    let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}")).await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT);
}

#[tokio::test]
async fn odd_answers_from_identity_are_a_502_not_a_leak() {
    for (label, auth_status, auth_body) in [
        (
            "500 from authenticate",
            StatusCode::INTERNAL_SERVER_ERROR,
            "boom: database password=hunter2",
        ),
        (
            "403 from authenticate (our own token refused)",
            StatusCode::FORBIDDEN,
            "{}",
        ),
        ("200 with nonsense", StatusCode::OK, "not json"),
        (
            "200 with an unknown permission",
            StatusCode::OK,
            r#"{"tenantId":"00000000-0000-0000-0000-00000000000b","userId":"00000000-0000-0000-0000-000000000016","keyId":"00000000-0000-0000-0000-000000000021","roles":[],"permissions":["root:everything"]}"#,
        ),
    ] {
        let body = auth_body.to_owned();
        let s = stub(move |_| (auth_status, body.clone(), Duration::ZERO)).await;
        let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
        let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}")).await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{label}");
        assert!(!r.text.contains("hunter2"), "{label}: internal text leaked");
        assert_eq!(
            s.calls().len(),
            1,
            "{label}: nothing may be forwarded after a failed authenticate"
        );
    }
}

#[tokio::test]
async fn service_errors_are_relayed_but_server_errors_and_refusals_of_our_token_are_not() {
    for (status, relayed) in [
        (StatusCode::CREATED, true),
        (StatusCode::BAD_REQUEST, true),
        (StatusCode::FORBIDDEN, true),
        (StatusCode::NOT_FOUND, true),
        (StatusCode::CONFLICT, true),
        (StatusCode::UNAUTHORIZED, false),
        (StatusCode::INTERNAL_SERVER_ERROR, false),
        (StatusCode::SERVICE_UNAVAILABLE, false),
    ] {
        let s = stub(move |s: &Seen| match s.path.as_str() {
            "/v1/authenticate" => (StatusCode::OK, principal_reply(), Duration::ZERO),
            _ => (status, r#"{"detail":"from identity"}"#.into(), Duration::ZERO),
        })
        .await;
        let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
        let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}")).await;
        if relayed {
            assert_eq!(r.status, status);
            assert!(r.text.contains("from identity"));
        } else {
            assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{status}");
            assert!(
                !r.text.contains("from identity"),
                "{status}: upstream body leaked"
            );
        }
    }
}

// ---------- limits ----------

#[tokio::test]
async fn bodies_over_64_kib_are_refused_after_authentication_and_never_forwarded() {
    let s = stub(happy).await;
    let gw = gateway(format!("http://{}", s.addr), Duration::from_secs(2));
    let huge = "x".repeat(64 * 1024 + 1);
    let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&huge)).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(s.calls().len(), 1, "only authenticate was called");
    // Unauthenticated: 401, not 413, so size limits are not an oracle.
    let r = call(&gw.app, Method::POST, "/v1/api-keys", &[], Some(&huge)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    // Exactly at the limit is fine.
    let ok = "x".repeat(64 * 1024);
    assert_eq!(
        post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&ok))
            .await
            .status,
        StatusCode::CREATED
    );
}
