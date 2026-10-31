// Written by Kone Tshivhinda & Claude
//! The gateway's authentication and routing against a *stub* identity service that records what it
//! receives. No database: these tests are about what the gateway forwards, refuses and hides.

use std::collections::HashMap;
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
    internal: Router,
    public_base64: String,
}

impl Gw {
    /// A verifier for tokens the gateway signed for `audience`.
    fn verifier(&self, audience: &str) -> Verifier {
        Verifier::from_config(audience, &format!("k1={}", self.public_base64)).unwrap()
    }
}

/// The default budgets, unless a test asks for a smaller one.
fn gateway(identity_url: String, timeout: Duration) -> Gw {
    gateway_with(identity_url, timeout, 600, 120)
}

fn gateway_with(identity_url: String, timeout: Duration, per_minute: u32, burst: u32) -> Gw {
    gateway_services(&[("identity", identity_url)], timeout, per_minute, burst)
}

fn gateway_services(urls: &[(&str, String)], timeout: Duration, per_minute: u32, burst: u32) -> Gw {
    let kp = generate_keypair().unwrap();
    let config = Config {
        signer: timelex_auth::Signer::from_base64("k1", &kp.private_pkcs8_base64).unwrap(),
        operator_key_hashes: vec![sha256(OPERATOR_KEY.as_str())],
        service_urls: urls
            .iter()
            .map(|(name, url)| ((*name).to_owned(), url.clone()))
            .collect(),
        upstream_timeout: timeout,
        rate_limit_per_minute: per_minute,
        rate_limit_burst: burst,
        internal_addr: "127.0.0.1:8089".parse().unwrap(),
    };
    let gateway = timelex_gateway::app_with(config).unwrap();
    Gw {
        app: gateway.public,
        internal: gateway.internal,
        public_base64: kp.public_base64,
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
    send(app, req.body(body).unwrap()).await
}

async fn send(app: &Router, req: Request) -> Reply {
    let r = app.clone().oneshot(req).await.unwrap();
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

async fn send_as(app: &Router, method: Method, path: &str, token: &str, body: Option<&str>) -> Reply {
    let b = bearer(token);
    call(app, method, path, &[("authorization", b[0].1.as_str())], body).await
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
        .verifier("identity")
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
        .verifier("identity")
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

// ---------- vault routes ----------

/// Identity and a vault sharing one stub: authenticate answers the principal, DELETE is a 204 like
/// the vault's revoke, everything else is a 2xx with a body.
fn vault_happy(s: &Seen) -> (StatusCode, String, Duration) {
    match (s.method.as_str(), s.path.as_str()) {
        ("POST", "/v1/authenticate") => (StatusCode::OK, principal_reply(), Duration::ZERO),
        ("DELETE", _) => (StatusCode::NO_CONTENT, String::new(), Duration::ZERO),
        _ => (StatusCode::OK, json!({"ok": true}).to_string(), Duration::ZERO),
    }
}

fn gateway_with_vault(url: &str) -> Gw {
    gateway_services(
        &[("identity", url.to_owned()), ("vault", url.to_owned())],
        Duration::from_secs(2),
        600,
        120,
    )
}

#[tokio::test]
async fn a_secret_call_is_authenticated_then_forwarded_to_the_vault_with_the_vault_audience() {
    let s = stub(vault_happy).await;
    let gw = gateway_with_vault(&format!("http://{}", s.addr));
    let body = json!({"value": "not-a-real-value", "label": "billing workspace"}).to_string();
    let r = send_as(
        &gw.app,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        TENANT_KEY.as_str(),
        Some(&body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.text,
        json!({"ok": true}).to_string(),
        "the vault's answer is relayed"
    );

    let calls = s.calls();
    assert_eq!(calls.len(), 2, "one authenticate, one forward");
    assert_eq!(
        (calls[0].method.as_str(), calls[0].path.as_str()),
        ("POST", "/v1/authenticate")
    );
    let fwd = &calls[1];
    assert_eq!(
        (fwd.method.as_str(), fwd.path.as_str()),
        ("PUT", "/v1/secrets/anthropic_api_key"),
        "the method and path are the vault contract's"
    );
    assert_eq!(fwd.body, body);

    // The token names the vault. A verifier for any other audience would refuse it, so success here
    // is what proves the audience.
    let verified = gw
        .verifier("vault")
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
        }
        other => panic!("expected a principal, got {other:?}"),
    }

    // The call is metered against the firm like any other.
    let usage = call(&gw.internal, Method::GET, "/internal/usage", &[], None).await;
    let usage: Value = serde_json::from_str(&usage.text).unwrap();
    assert_eq!(usage["tenants"][Uuid::from_u128(11).to_string()]["requests"], 1);
}

#[tokio::test]
async fn secret_paths_are_canonicalised_and_bad_parameters_are_refused_after_authenticating() {
    let s = stub(vault_happy).await;
    let gw = gateway_with_vault(&format!("http://{}", s.addr));
    let key = TENANT_KEY.as_str();

    // A UUID reaches the vault in canonical form, whatever case the client used.
    let id = Uuid::new_v4();
    let upper = id.to_string().to_uppercase();
    let r = send_as(
        &gw.app,
        Method::DELETE,
        &format!("/v1/secrets/{upper}"),
        key,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(s.calls().last().unwrap().path, format!("/v1/secrets/{id}"));

    let r = post_as(
        &gw.app,
        &format!("/v1/secrets/{upper}/rotate"),
        key,
        Some(&json!({"value": "next"}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(s.calls().last().unwrap().path, format!("/v1/secrets/{id}/rotate"));

    // The list query is forwarded only in its canonical form.
    let b = bearer(key);
    let auth = [("authorization", b[0].1.as_str())];
    let r = call(
        &gw.app,
        Method::GET,
        "/v1/secrets?includeRevoked=true",
        &auth,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(s.calls().last().unwrap().path, "/v1/secrets?includeRevoked=true");
    let before = s.calls().len();
    let r = call(
        &gw.app,
        Method::GET,
        "/v1/secrets?tenantId=someone-else",
        &auth,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(s.calls().len(), before + 1, "authenticated, then refused");

    // A kind the vault does not define, and ids that are not UUIDs, never reach it.
    for (method, path, body) in [
        (Method::PUT, "/v1/secrets/admin_key", Some("{}")),
        (Method::DELETE, "/v1/secrets/not-a-uuid", None),
        (Method::POST, "/v1/secrets/not-a-uuid/test", None),
    ] {
        let before = s.calls().len();
        let r = send_as(&gw.app, method, path, key, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(s.calls().len(), before + 1, "{path}: only authenticate");
    }

    // Unauthenticated callers get the 401 before any parameter is looked at.
    let r = call(&gw.app, Method::POST, "/v1/secrets/not-a-uuid/test", &[], None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_secret_call_with_no_vault_configured_is_a_502_and_nothing_is_forwarded() {
    let s = stub(vault_happy).await;
    let gw = gateway_services(
        &[("identity", format!("http://{}", s.addr))],
        Duration::from_secs(2),
        600,
        120,
    );
    let r = send_as(
        &gw.app,
        Method::PUT,
        "/v1/secrets/anthropic_api_key",
        TENANT_KEY.as_str(),
        Some(&json!({"value": "not-a-real-value"}).to_string()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        s.calls().len(),
        1,
        "authenticated, and no call reached a service route"
    );
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
        .verifier("identity")
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
        service_urls: HashMap::from([("identity".to_owned(), format!("http://{}", s.addr))]),
        upstream_timeout: Duration::from_secs(2),
        rate_limit_per_minute: 600,
        rate_limit_burst: 120,
        internal_addr: "127.0.0.1:8089".parse().unwrap(),
    };
    let app = timelex_gateway::app_with(config).unwrap().public;
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

// ---------- request budget ----------

#[tokio::test]
async fn a_firm_over_its_budget_gets_429_and_nothing_is_forwarded() {
    let s = stub(happy).await;
    let gw = gateway_with(format!("http://{}", s.addr), Duration::from_secs(2), 60, 2);
    let body = json!({"name": "k", "scopes": ["time:read"]}).to_string();
    for _ in 0..2 {
        assert_eq!(
            post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&body))
                .await
                .status,
            StatusCode::CREATED
        );
    }
    assert_eq!(s.calls().len(), 4, "two authenticate calls and two forwards");

    let r = post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&body)).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        r.headers[header::RETRY_AFTER],
        "1",
        "60 a minute refills one a second"
    );
    assert!(r.text.contains("urn:timelex:problem:rate-limited"));
    // The bucket is keyed on who identity confirmed, so authentication still happens; the request
    // itself is never forwarded.
    assert_eq!(s.calls().len(), 5, "one authenticate call, no forward");
    assert_eq!(s.calls().last().unwrap().path, "/v1/authenticate");
    assert_eq!(
        s.calls().iter().filter(|c| c.path != "/v1/authenticate").count(),
        2,
        "nothing beyond the two accepted calls reached a service route"
    );
}

#[tokio::test]
async fn the_operator_budget_is_separate_from_the_firm_budget() {
    let s = stub(happy).await;
    let gw = gateway_with(format!("http://{}", s.addr), Duration::from_secs(2), 60, 1);
    assert_eq!(
        post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}"))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(
        post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}"))
            .await
            .status,
        StatusCode::TOO_MANY_REQUESTS
    );
    let body = json!({"slug": "example-attorneys"}).to_string();
    assert_eq!(
        post_as(&gw.app, "/v1/tenants", OPERATOR_KEY.as_str(), Some(&body))
            .await
            .status,
        StatusCode::CREATED,
        "one firm's exhaustion does not touch the operator"
    );
}

// ---------- usage ----------

#[tokio::test]
async fn usage_totals_are_served_on_the_internal_router_only() {
    let s = stub(happy).await;
    let gw = gateway_with(format!("http://{}", s.addr), Duration::from_secs(2), 60, 1);
    let body = json!({"name": "k", "scopes": ["time:read"]}).to_string();
    assert_eq!(
        post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&body))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(
        post_as(&gw.app, "/v1/api-keys", TENANT_KEY.as_str(), Some(&body))
            .await
            .status,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        call(&gw.app, Method::POST, "/v1/api-keys", &[], Some("{}"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    // The public listener knows nothing about usage.
    assert_eq!(
        call(&gw.app, Method::GET, "/internal/usage", &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let r = call(&gw.internal, Method::GET, "/internal/usage", &[], None).await;
    assert_eq!(r.status, StatusCode::OK);
    let usage: Value = serde_json::from_str(&r.text).unwrap();
    assert_eq!(usage["totals"]["requests"], 3);
    assert_eq!(usage["totals"]["rateLimited"], 1);
    assert_eq!(usage["totals"]["unidentified"], 1);
    assert!(usage["totals"]["bytesIn"].as_u64().unwrap() > 0);
    assert!(usage["totals"]["bytesOut"].as_u64().unwrap() > 0);
    let firm = Uuid::from_u128(11).to_string();
    assert_eq!(
        usage["tenants"][&firm]["requests"], 2,
        "the 429 is attributed to the firm too"
    );
    assert_eq!(usage["tenants"][&firm]["rateLimited"], 1);
    assert_eq!(usage["operator"]["requests"], 0);
    assert!(usage["since"].as_str().unwrap().ends_with('Z'));
    assert!(usage["uptimeSeconds"].is_u64());

    // The internal listener serves nothing else.
    assert_eq!(
        post_as(&gw.internal, "/v1/api-keys", TENANT_KEY.as_str(), Some("{}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

// ---------- MCP routing ----------

async fn mcp_call(app: &Router, token: &str, headers: &[(&str, &str)], body: &str) -> Reply {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri("/mcp")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    send(app, req.body(Body::from(body.to_owned())).unwrap()).await
}

fn mcp_headers<'a>(method: &'a str, name: Option<&'a str>) -> Vec<(&'a str, &'a str)> {
    let mut headers = vec![("mcp-protocol-version", "2026-07-28"), ("mcp-method", method)];
    if let Some(name) = name {
        headers.push(("mcp-name", name));
    }
    headers
}

fn tools_call_body(name: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {"name": name, "arguments": {"limit": 5}}
    })
    .to_string()
}

#[tokio::test]
async fn a_tools_call_reaches_the_owning_service_with_the_protocol_headers() {
    let identity = stub(happy).await;
    let answer = json!({"jsonrpc": "2.0", "id": 7, "result": {"ok": true}}).to_string();
    let service_answer = answer.clone();
    let vault = stub(move |_| (StatusCode::OK, service_answer.clone(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        600,
        120,
    );

    let body = tools_call_body("list_secrets");
    let mut headers = mcp_headers("tools/call", Some("list_secrets"));
    headers.push(("content-type", "application/json"));
    headers.push(("origin", "https://app.timelex.example"));
    headers.push(("cookie", "session=abc"));
    headers.push(("x-timelex-tenant", "another-firm"));
    let r = mcp_call(&gw.app, TENANT_KEY.as_str(), &headers, &body).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.text, answer, "the service's answer is relayed");

    // Identity authenticated the key; the tool call itself went only to the owning service.
    assert_eq!(identity.calls().len(), 1);
    assert_eq!(identity.calls()[0].path, "/v1/authenticate");
    let calls = vault.calls();
    assert_eq!(calls.len(), 1);
    let fwd = &calls[0];
    assert_eq!((fwd.method.as_str(), fwd.path.as_str()), ("POST", "/mcp"));
    assert_eq!(fwd.body, body, "the body is forwarded untouched");

    // The token names the service it was minted for, not identity.
    let who = gw
        .verifier("vault")
        .verify(
            fwd.header("authorization")
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap(),
        )
        .unwrap();
    match who.caller {
        Caller::Principal(p) => {
            assert_eq!(p.tenant.as_uuid(), Uuid::from_u128(11));
            assert_eq!(p.user.as_uuid(), Uuid::from_u128(22));
            assert_eq!(p.key.as_uuid(), Uuid::from_u128(33));
        }
        other => panic!("expected a principal, got {other:?}"),
    }

    assert_eq!(fwd.header("mcp-protocol-version"), Some("2026-07-28"));
    assert_eq!(fwd.header("mcp-method"), Some("tools/call"));
    assert_eq!(fwd.header("mcp-name"), Some("list_secrets"));
    assert_eq!(fwd.header("origin"), Some("https://app.timelex.example"));
    assert_eq!(fwd.header("content-type"), Some("application/json"));

    // An allowlist, as for the REST routes: nothing else the client sent travels on. (`accept` is
    // `*/*`, added by the HTTP client, not the caller.)
    let allowed = [
        "host",
        "authorization",
        "content-type",
        "content-length",
        "accept",
        "mcp-protocol-version",
        "mcp-method",
        "mcp-name",
        "origin",
    ];
    for (name, value) in &fwd.headers {
        assert!(
            allowed.contains(&name.as_str()),
            "unexpected header reached the service: {name}"
        );
        assert!(
            !value.contains(TENANT_KEY.as_str()),
            "the API key reached the service in {name}"
        );
    }
    assert!(fwd.header("cookie").is_none());
    assert!(fwd.header("x-timelex-tenant").is_none());

    // The call is metered like any other handled request.
    let usage = call(&gw.internal, Method::GET, "/internal/usage", &[], None).await;
    let usage: Value = serde_json::from_str(&usage.text).unwrap();
    assert_eq!(usage["totals"]["requests"], 1);
}

#[tokio::test]
async fn an_identity_owned_tool_goes_to_identity_and_not_elsewhere() {
    let identity = stub(happy).await;
    let gw = gateway(format!("http://{}", identity.addr), Duration::from_secs(2));
    let body = tools_call_body("list_users");
    let r = mcp_call(
        &gw.app,
        TENANT_KEY.as_str(),
        &mcp_headers("tools/call", Some("list_users")),
        &body,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "identity's answer is relayed");
    let calls = identity.calls();
    assert_eq!(calls.len(), 2, "authenticate, then the tool call");
    assert_eq!(calls[1].path, "/mcp");
    assert_eq!(calls[1].body, body);
}

#[tokio::test]
async fn an_unknown_tool_is_a_404_and_nothing_is_forwarded() {
    let identity = stub(happy).await;
    let vault = stub(|_| (StatusCode::OK, "{}".into(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        600,
        120,
    );
    let r = mcp_call(
        &gw.app,
        TENANT_KEY.as_str(),
        &mcp_headers("tools/call", Some("delete_everything")),
        &tools_call_body("delete_everything"),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(r.text.contains("urn:timelex:problem:not-found"));
    assert_eq!(identity.calls().len(), 1, "only authenticate was called");
    assert!(vault.calls().is_empty());
}

#[tokio::test]
async fn valid_methods_this_version_does_not_route_are_501() {
    let identity = stub(happy).await;
    let vault = stub(|_| (StatusCode::OK, "{}".into(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        600,
        120,
    );
    for method in ["tools/list", "server/discover", "tasks/get", "prompts/get"] {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method}).to_string();
        let r = mcp_call(&gw.app, TENANT_KEY.as_str(), &mcp_headers(method, None), &body).await;
        assert_eq!(r.status, StatusCode::NOT_IMPLEMENTED, "{method}");
        assert!(
            r.text.contains("urn:timelex:problem:not-implemented"),
            "{method}: {}",
            r.text
        );
    }
    assert!(vault.calls().is_empty(), "nothing unroutable was forwarded");
}

#[tokio::test]
async fn a_tool_whose_service_has_no_url_is_501() {
    let identity = stub(happy).await;
    // Only identity is configured; the table still names vault for this tool.
    let gw = gateway(format!("http://{}", identity.addr), Duration::from_secs(2));
    let r = mcp_call(
        &gw.app,
        TENANT_KEY.as_str(),
        &mcp_headers("tools/call", Some("list_secrets")),
        &tools_call_body("list_secrets"),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_IMPLEMENTED);
    assert!(r.text.contains("urn:timelex:problem:not-implemented"));
    assert_eq!(identity.calls().len(), 1, "nothing can be forwarded");
}

#[tokio::test]
async fn malformed_mcp_headers_are_400_after_authentication_and_401_before_it() {
    let identity = stub(happy).await;
    let vault = stub(|_| (StatusCode::OK, "{}".into(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        600,
        120,
    );
    let body = tools_call_body("list_secrets");
    let name = Some("list_secrets");
    let bad = [
        (
            "missing version",
            vec![("mcp-method", "tools/call"), ("mcp-name", "list_secrets")],
            "MCP-Protocol-Version",
        ),
        (
            "wrong version",
            vec![
                ("mcp-protocol-version", "2025-11-25"),
                ("mcp-method", "tools/call"),
                ("mcp-name", "list_secrets"),
            ],
            "MCP-Protocol-Version",
        ),
        (
            "missing method",
            vec![
                ("mcp-protocol-version", "2026-07-28"),
                ("mcp-name", "list_secrets"),
            ],
            "Mcp-Method",
        ),
        (
            "call without a name",
            vec![
                ("mcp-protocol-version", "2026-07-28"),
                ("mcp-method", "tools/call"),
            ],
            "Mcp-Name",
        ),
        (
            "name with a space",
            vec![
                ("mcp-protocol-version", "2026-07-28"),
                ("mcp-method", "tools/call"),
                ("mcp-name", "list secrets"),
            ],
            "Mcp-Name",
        ),
    ];
    for (label, headers, detail) in bad {
        let r = mcp_call(&gw.app, TENANT_KEY.as_str(), &headers, &body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{label}");
        assert!(r.text.contains(detail), "{label}: {}", r.text);
    }
    // A repeated header is ambiguous, so it is refused however plausible each value looks.
    let mut duplicated = mcp_headers("tools/call", name);
    duplicated.push(("mcp-name", "list_secrets"));
    let r = mcp_call(&gw.app, TENANT_KEY.as_str(), &duplicated, &body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Authentication comes first: an unauthenticated caller learns only that it is unauthenticated.
    let before = identity.calls().len();
    let r = call(
        &gw.app,
        Method::POST,
        "/mcp",
        &mcp_headers("tools/call", Some("list_secrets")),
        Some(&body),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(identity.calls().len(), before, "no key, no authenticate call");
    assert!(vault.calls().is_empty(), "nothing malformed was forwarded");
}

#[tokio::test]
async fn an_operator_key_is_not_an_mcp_credential() {
    // Identity refuses the operator key, exactly as it refuses any key it issued to nobody.
    let identity = stub(|s: &Seen| {
        if s.body.contains(OPERATOR_KEY.as_str()) {
            (StatusCode::UNAUTHORIZED, "{}".into(), Duration::ZERO)
        } else {
            (StatusCode::OK, principal_reply(), Duration::ZERO)
        }
    })
    .await;
    let vault = stub(|_| (StatusCode::OK, "{}".into(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        600,
        120,
    );
    let r = mcp_call(
        &gw.app,
        OPERATOR_KEY.as_str(),
        &mcp_headers("tools/call", Some("list_secrets")),
        &tools_call_body("list_secrets"),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        identity.calls().len(),
        1,
        "it was offered to identity as an API key and refused"
    );
    assert!(vault.calls().is_empty());
}

#[tokio::test]
async fn the_firm_budget_also_covers_mcp_calls() {
    let identity = stub(happy).await;
    let vault = stub(|_| (StatusCode::OK, "{}".into(), Duration::ZERO)).await;
    let gw = gateway_services(
        &[
            ("identity", format!("http://{}", identity.addr)),
            ("vault", format!("http://{}", vault.addr)),
        ],
        Duration::from_secs(2),
        60,
        1,
    );
    let headers = mcp_headers("tools/call", Some("list_secrets"));
    let body = tools_call_body("list_secrets");
    assert_eq!(
        mcp_call(&gw.app, TENANT_KEY.as_str(), &headers, &body)
            .await
            .status,
        StatusCode::OK
    );
    let r = mcp_call(&gw.app, TENANT_KEY.as_str(), &headers, &body).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.headers[header::RETRY_AFTER], "1");
    assert_eq!(vault.calls().len(), 1, "the refused call was not forwarded");
}
