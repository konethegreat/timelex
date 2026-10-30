// Written by Kone Tshivhinda & Claude
//! Authentication and routing: `contracts/openapi/gateway.v1.yaml`, ADR-0012 and ADR-0013.
//!
//! The gateway never forwards what a client sent. It checks the caller, then builds a *new* request
//! to the service carrying only an internal token and the body. So no header a client chose (an
//! `X-TimeLex-Tenant`, a cookie, the API key itself) can reach a service.
//!
//! Every handled request is metered once, whatever the outcome, and a firm over its budget is refused
//! with 429 before its body is read.

use std::sync::Arc;
use std::time::Instant;

use axum::body::{to_bytes, Body, Bytes};
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use timelex_auth::{Caller, Principal};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use uuid::Uuid;

use crate::config::{sha256, Config, OPERATOR_RATE_LIMIT_BURST, OPERATOR_RATE_LIMIT_PER_MINUTE};
use crate::limits::{Decision, RateKey, RateLimiter};
use crate::mcp::{self, Table};
use crate::meter::{now_rfc3339, CallerKind, Meter, UsageRecord, UsageSnapshot};

/// Largest request body the gateway will read and forward.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

const MCP_VERSION: header::HeaderName = header::HeaderName::from_static("mcp-protocol-version");
const MCP_METHOD: header::HeaderName = header::HeaderName::from_static("mcp-method");
const MCP_NAME: header::HeaderName = header::HeaderName::from_static("mcp-name");

struct Inner {
    config: Config,
    http: reqwest::Client,
    limiter: RateLimiter,
    meter: Meter,
    tools: Table,
}

/// Shared gateway state: configuration, one pooled HTTP client, the buckets and the meter.
#[derive(Clone)]
pub struct GatewayState(Arc<Inner>);

impl GatewayState {
    /// Build the state. The client follows no redirects and ignores proxy environment variables:
    /// internal calls must go exactly where the configuration says.
    pub fn new(config: Config) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(config.upstream_timeout)
            .connect_timeout(config.upstream_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|e| format!("cannot build the HTTP client: {e}"))?;
        let limiter = RateLimiter::new(
            config.rate_limit_per_minute,
            config.rate_limit_burst,
            OPERATOR_RATE_LIMIT_PER_MINUTE,
            OPERATOR_RATE_LIMIT_BURST,
        );
        Ok(Self(Arc::new(Inner {
            config,
            http,
            limiter,
            meter: Meter::stdout(),
            tools: Table::built_in()?,
        })))
    }

    /// The running usage totals, for the internal listener.
    pub fn usage(&self) -> UsageSnapshot {
        self.0.meter.snapshot()
    }
}

/// The routes this version forwards, and who may use each. `authenticate` is deliberately absent.
pub fn routes(state: GatewayState) -> Router {
    Router::new()
        .route("/v1/tenants", post(create_tenant))
        .route("/v1/api-keys", post(issue_api_key))
        .route("/v1/api-keys/{key_id}/revoke", post(revoke_api_key))
        .route("/mcp", post(mcp))
        .with_state(state)
}

/// The internal listener's only endpoint. It must be bound to loopback: these totals are for operators,
/// not tenants.
pub fn internal_routes(state: GatewayState) -> Router {
    Router::new()
        .route("/internal/usage", get(usage))
        .fallback(|| async { problem(StatusCode::NOT_FOUND, "not-found", "Not found", None) })
        .with_state(state)
}

async fn usage(State(state): State<GatewayState>) -> Json<UsageSnapshot> {
    Json(state.usage())
}

// ---------- problem details ----------

/// Why the gateway answered itself instead of relaying a service's answer.
enum Reject {
    Unauthorized,
    Invalid(&'static str),
    NotFound(&'static str),
    NotImplemented(&'static str),
    TooLarge,
    RateLimited(u64),
    BadGateway,
    GatewayTimeout,
    Internal,
}

pub(crate) fn problem(status: StatusCode, slug: &str, title: &str, detail: Option<&str>) -> Response {
    let mut body =
        json!({"type": format!("urn:timelex:problem:{slug}"), "title": title, "status": status.as_u16()});
    if let Some(d) = detail {
        body["detail"] = d.into();
    }
    let mut r = (status, Json(body)).into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    if status == StatusCode::UNAUTHORIZED {
        r.headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    }
    r
}

impl IntoResponse for Reject {
    fn into_response(self) -> Response {
        match self {
            Reject::Unauthorized => {
                problem(StatusCode::UNAUTHORIZED, "unauthenticated", "Unauthorized", None)
            }
            Reject::Invalid(d) => problem(
                StatusCode::BAD_REQUEST,
                "invalid-request",
                "Invalid request",
                Some(d),
            ),
            Reject::NotFound(d) => problem(StatusCode::NOT_FOUND, "not-found", "Not found", Some(d)),
            Reject::NotImplemented(d) => problem(
                StatusCode::NOT_IMPLEMENTED,
                "not-implemented",
                "Not implemented",
                Some(d),
            ),
            Reject::TooLarge => problem(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too-large",
                "Request too large",
                None,
            ),
            Reject::RateLimited(seconds) => {
                let mut r = problem(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate-limited",
                    "Too many requests",
                    Some("the request budget is exhausted; retry after the given number of seconds"),
                );
                r.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from(seconds.max(1)));
                r
            }
            Reject::BadGateway => problem(StatusCode::BAD_GATEWAY, "bad-gateway", "Bad gateway", None),
            Reject::GatewayTimeout => problem(
                StatusCode::GATEWAY_TIMEOUT,
                "gateway-timeout",
                "Gateway timeout",
                None,
            ),
            Reject::Internal => problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal error",
                None,
            ),
        }
    }
}

fn upstream_failure(e: &reqwest::Error) -> Reject {
    // A refused or dropped connection is a 502 (the service was not reached). A timeout is a 504 (no
    // answer in time). Caveat found by testing: reqwest cannot tell a *connect* timeout from a response
    // timeout, and on Windows a closed port never refuses, it just times out, so there it reads as 504.
    // Both mean "nothing was changed or the outcome is unknown", which is what the contract tells callers.
    if e.is_connect() {
        Reject::BadGateway
    } else if e.is_timeout() {
        Reject::GatewayTimeout
    } else {
        Reject::BadGateway
    }
}

// ---------- credentials ----------

/// Exactly one value for `name`, or `None` when it is absent or repeated. Accepts no second opinion:
/// a header the gateway routes on must be unambiguous, since the service will re-check it.
fn single_header<'a>(headers: &'a HeaderMap, name: &header::HeaderName) -> Option<&'a str> {
    let mut all = headers.get_all(name).iter();
    let (first, rest) = (all.next()?, all.next());
    if rest.is_some() {
        return None;
    }
    first.to_str().ok()
}

/// The one bearer token in the request, or `None` for anything else (missing, repeated, another
/// scheme, empty, absurdly long). Callers answer `None` with the same 401 as a wrong key.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let (scheme, token) = single_header(headers, &header::AUTHORIZATION)?.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() && token.len() <= 200 && !token.contains(' '))
        .then_some(token)
}

/// Compare two hashes without stopping at the first difference.
fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl GatewayState {
    fn is_operator(&self, presented: &str) -> bool {
        let h = sha256(presented);
        let mut ok = false;
        for known in &self.0.config.operator_key_hashes {
            ok |= ct_eq(&h, known); // `|=` on purpose: every configured key is always checked
        }
        ok
    }

    /// Ask identity who this API key is. A 401 from identity means "not a usable key"; anything
    /// odd (5xx, an unreadable answer, a refusal of our own service token) is the gateway's problem.
    async fn authenticate(&self, api_key: &str) -> Result<Principal, Reject> {
        let base = self.0.config.service_url("identity").ok_or(Reject::Internal)?;
        let token = self
            .0
            .config
            .signer
            .sign("identity", &Caller::Service)
            .map_err(|_| Reject::Internal)?;
        let response = self
            .0
            .http
            .post(format!("{base}/v1/authenticate"))
            .bearer_auth(token)
            .header(header::CONTENT_TYPE, "application/json")
            .body(json!({ "token": api_key }).to_string())
            .send()
            .await
            .map_err(|e| upstream_failure(&e))?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => return Err(Reject::Unauthorized),
            _ => return Err(Reject::BadGateway),
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Reply {
            tenant_id: Uuid,
            user_id: Uuid,
            key_id: Uuid,
            permissions: Vec<String>,
        }
        let bytes = response.bytes().await.map_err(|e| upstream_failure(&e))?;
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| Reject::BadGateway)?;
        let permissions = reply
            .permissions
            .iter()
            .map(|p| Permission::parse(p))
            .collect::<Result<_, _>>()
            .map_err(|_| Reject::BadGateway)?;
        Ok(Principal {
            tenant: TenantId::from_uuid(reply.tenant_id),
            user: UserId::from_uuid(reply.user_id),
            key: ApiKeyId::from_uuid(reply.key_id),
            permissions,
        })
    }

    /// Identify the firm behind the presented API key. Used by the tenant-facing REST routes and by
    /// `/mcp`. A 401 here means the key is not usable; nothing else is the caller's business.
    async fn tenant_principal(&self, headers: &HeaderMap) -> Result<Principal, Reject> {
        let presented = bearer(headers).ok_or(Reject::Unauthorized)?;
        self.authenticate(presented).await
    }

    /// Identify the caller and mint the internal token to forward to identity with.
    async fn internal_token(&self, who: Who, headers: &HeaderMap) -> Result<(Caller, String), Reject> {
        let caller = match who {
            Who::Operator => {
                let presented = bearer(headers).ok_or(Reject::Unauthorized)?;
                if !self.is_operator(presented) {
                    return Err(Reject::Unauthorized);
                }
                Caller::Operator
            }
            Who::Tenant => Caller::Principal(self.tenant_principal(headers).await?),
        };
        let token = self
            .0
            .config
            .signer
            .sign("identity", &caller)
            .map_err(|_| Reject::Internal)?;
        Ok((caller, token))
    }

    /// Send a fresh request to identity and relay the answer, with the size of the answer.
    async fn forward(
        &self,
        path: &str,
        token: String,
        body: Option<Bytes>,
    ) -> Result<(Response, u64), Reject> {
        let base = self.0.config.service_url("identity").ok_or(Reject::Internal)?;
        let mut request = self.0.http.post(format!("{base}{path}")).bearer_auth(token);
        if let Some(b) = body.filter(|b| !b.is_empty()) {
            request = request.header(header::CONTENT_TYPE, "application/json").body(b);
        }
        let upstream = request.send().await.map_err(|e| upstream_failure(&e))?;
        relay_response(upstream).await
    }

    /// Send a request to an already-resolved URL with an allow-list of headers, and relay the answer.
    async fn relay(
        &self,
        url: &str,
        token: String,
        body: Bytes,
        headers: &[(&'static str, String)],
    ) -> Result<(Response, u64), Reject> {
        let mut request = self.0.http.post(url).bearer_auth(token);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let upstream = request
            .body(body)
            .send()
            .await
            .map_err(|e| upstream_failure(&e))?;
        relay_response(upstream).await
    }
}

/// Relay a service's answer, with its size. A 401 means the service refused *our* token
/// (misconfiguration) and a 5xx is its own failure; neither is the client's business, so neither is
/// relayed and both become a 502.
async fn relay_response(upstream: reqwest::Response) -> Result<(Response, u64), Reject> {
    let status = upstream.status();
    if status == StatusCode::UNAUTHORIZED || status.is_server_error() {
        return Err(Reject::BadGateway);
    }
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let cache_control = upstream.headers().get(header::CACHE_CONTROL).cloned();
    let bytes = upstream.bytes().await.map_err(|e| upstream_failure(&e))?;
    let len = bytes.len() as u64;
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(v) = content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    if let Some(v) = cache_control {
        response.headers_mut().insert(header::CACHE_CONTROL, v);
    }
    Ok((response, len))
}

#[derive(Clone, Copy)]
enum Who {
    Operator,
    Tenant,
}

/// The clock and labels for one request, so every exit path records exactly one line.
struct Trip {
    label: &'static str,
    method: Method,
    started: Instant,
}

impl Trip {
    fn new(label: &'static str, method: Method) -> Self {
        Self {
            label,
            method,
            started: Instant::now(),
        }
    }

    /// Record the request as it ended. `caller` is `None` when authentication failed; `service` and
    /// `tool` are `None` when the request was refused before routing named them.
    #[allow(clippy::too_many_arguments)]
    fn done(
        &self,
        state: &GatewayState,
        caller: Option<&Caller>,
        service: Option<&str>,
        tool: Option<&str>,
        status: StatusCode,
        bytes_in: u64,
        bytes_out: u64,
        rate_limited: bool,
    ) {
        let (kind, tenant, user, key) = match caller {
            Some(Caller::Principal(p)) => (
                CallerKind::Principal,
                Some(p.tenant.as_uuid()),
                Some(p.user.as_uuid()),
                Some(p.key.as_uuid()),
            ),
            Some(Caller::Operator) => (CallerKind::Operator, None, None, None),
            Some(Caller::Service) => (CallerKind::Service, None, None, None),
            None => (CallerKind::Unidentified, None, None, None),
        };
        state.0.meter.record(&UsageRecord {
            at: now_rfc3339(),
            caller: kind,
            tenant_id: tenant,
            user_id: user,
            key_id: key,
            route: self.label,
            service,
            tool,
            method: self.method.to_string(),
            status: status.as_u16(),
            duration_ms: self.started.elapsed().as_millis() as u64,
            bytes_in,
            bytes_out,
            rate_limited,
        });
    }
}

/// Authenticate, check the budget, then read the (size-limited) body and forward. Authentication comes
/// first so an unauthenticated caller learns nothing about limits or routes; the budget comes before
/// the body so a firm at its limit does not make the gateway read anything.
///
/// `path` is `None` when the route's parameter is malformed: the caller is authenticated first (so the
/// answer never depends on who is asking) and then gets a 400.
async fn handle(
    state: GatewayState,
    who: Who,
    label: &'static str,
    path: Option<String>,
    req: Request,
) -> Response {
    let trip = Trip::new(label, req.method().clone());
    let (caller, token) = match state.internal_token(who, req.headers()).await {
        Ok(identified) => identified,
        Err(reject) => {
            let response = reject.into_response();
            trip.done(&state, None, None, None, response.status(), 0, 0, false);
            return response;
        }
    };
    let key = match &caller {
        Caller::Principal(p) => RateKey::Tenant(p.tenant.as_uuid()),
        // Public routes mint only operator and principal tokens. A service token would share the
        // operator's budget rather than escape limiting.
        Caller::Operator | Caller::Service => RateKey::Operator,
    };
    if let Decision {
        allowed: false,
        retry_after_seconds,
    } = state.0.limiter.check(key, Instant::now())
    {
        let response = Reject::RateLimited(retry_after_seconds).into_response();
        trip.done(&state, Some(&caller), None, None, response.status(), 0, 0, true);
        return response;
    }
    let Some(path) = path else {
        let response = Reject::Invalid("keyId is not a UUID").into_response();
        trip.done(&state, Some(&caller), None, None, response.status(), 0, 0, false);
        return response;
    };
    let body = match to_bytes(req.into_body(), MAX_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            let response = Reject::TooLarge.into_response();
            // The read stopped at the limit, so the size is unknown but at least one byte more.
            trip.done(
                &state,
                Some(&caller),
                Some("identity"),
                None,
                response.status(),
                MAX_BODY_BYTES as u64 + 1,
                0,
                false,
            );
            return response;
        }
    };
    let bytes_in = body.len() as u64;
    match state.forward(&path, token, Some(body)).await {
        Ok((response, bytes_out)) => {
            trip.done(
                &state,
                Some(&caller),
                Some("identity"),
                None,
                response.status(),
                bytes_in,
                bytes_out,
                false,
            );
            response
        }
        Err(reject) => {
            let response = reject.into_response();
            trip.done(
                &state,
                Some(&caller),
                Some("identity"),
                None,
                response.status(),
                bytes_in,
                0,
                false,
            );
            response
        }
    }
}

async fn create_tenant(State(s): State<GatewayState>, req: Request) -> Response {
    handle(
        s,
        Who::Operator,
        "tenants.create",
        Some("/v1/tenants".into()),
        req,
    )
    .await
}

async fn issue_api_key(State(s): State<GatewayState>, req: Request) -> Response {
    handle(s, Who::Tenant, "apiKeys.issue", Some("/v1/api-keys".into()), req).await
}

async fn revoke_api_key(State(s): State<GatewayState>, Path(key_id): Path<String>, req: Request) -> Response {
    // Re-format the id ourselves: only a canonical UUID ever reaches the service's URL.
    let path = Uuid::parse_str(&key_id)
        .ok()
        .map(|key| format!("/v1/api-keys/{}/revoke", key.hyphenated()));
    handle(s, Who::Tenant, "apiKeys.revoke", path, req).await
}

// ---------- MCP ----------

/// Route one MCP request from its headers. The gateway names the owning service from the table and
/// forwards the body untouched, carrying only the headers the protocol needs and the `Origin` the
/// service must re-check. It never parses the body; the service does, and re-checks these headers.
async fn mcp(State(state): State<GatewayState>, req: Request) -> Response {
    let trip = Trip::new("mcp.call", req.method().clone());
    let principal = match state.tenant_principal(req.headers()).await {
        Ok(principal) => principal,
        Err(reject) => {
            let response = reject.into_response();
            trip.done(&state, None, None, None, response.status(), 0, 0, false);
            return response;
        }
    };
    let caller = Caller::Principal(principal.clone());
    let key = RateKey::Tenant(principal.tenant.as_uuid());
    if let Decision {
        allowed: false,
        retry_after_seconds,
    } = state.0.limiter.check(key, Instant::now())
    {
        let response = Reject::RateLimited(retry_after_seconds).into_response();
        trip.done(&state, Some(&caller), None, None, response.status(), 0, 0, true);
        return response;
    }

    let headers = req.headers();
    let tool = match mcp::classify(
        single_header(headers, &MCP_VERSION),
        single_header(headers, &MCP_METHOD),
        single_header(headers, &MCP_NAME),
    ) {
        Ok(mcp::Outcome::ToolCall(tool)) => tool.to_owned(),
        Ok(mcp::Outcome::Unrouted) => {
            let response = Reject::NotImplemented("this MCP method is not routed yet").into_response();
            trip.done(&state, Some(&caller), None, None, response.status(), 0, 0, false);
            return response;
        }
        Err(detail) => {
            let response = Reject::Invalid(detail).into_response();
            trip.done(&state, Some(&caller), None, None, response.status(), 0, 0, false);
            return response;
        }
    };
    let Some(service) = state.0.tools.service_for(&tool).map(str::to_owned) else {
        let response = Reject::NotFound("the tool is not in the routing table").into_response();
        trip.done(
            &state,
            Some(&caller),
            None,
            Some(&tool),
            response.status(),
            0,
            0,
            false,
        );
        return response;
    };
    let Some(base) = state.0.config.service_url(&service) else {
        let response = Reject::NotImplemented("the tool's service has no configured URL").into_response();
        trip.done(
            &state,
            Some(&caller),
            Some(&service),
            Some(&tool),
            response.status(),
            0,
            0,
            false,
        );
        return response;
    };
    let token = match state.0.config.signer.sign(&service, &caller) {
        Ok(token) => token,
        Err(_) => {
            let response = Reject::Internal.into_response();
            trip.done(
                &state,
                Some(&caller),
                Some(&service),
                Some(&tool),
                response.status(),
                0,
                0,
                false,
            );
            return response;
        }
    };

    // Only these headers are rebuilt for the service; nothing else the client sent travels on.
    let mut extra: Vec<(&'static str, String)> = Vec::with_capacity(6);
    let content_type = single_header(headers, &header::CONTENT_TYPE).unwrap_or("application/json");
    extra.push(("content-type", content_type.to_owned()));
    extra.push(("mcp-protocol-version", mcp::PROTOCOL_VERSION.to_owned()));
    extra.push(("mcp-method", "tools/call".to_owned()));
    extra.push(("mcp-name", tool.clone()));
    if let Some(origin) = single_header(headers, &header::ORIGIN) {
        extra.push(("origin", origin.to_owned()));
    }

    let body = match to_bytes(req.into_body(), MAX_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            let response = Reject::TooLarge.into_response();
            trip.done(
                &state,
                Some(&caller),
                Some(&service),
                Some(&tool),
                response.status(),
                MAX_BODY_BYTES as u64 + 1,
                0,
                false,
            );
            return response;
        }
    };
    let bytes_in = body.len() as u64;
    let url = format!("{base}/mcp");
    match state.relay(&url, token, body, &extra).await {
        Ok((response, bytes_out)) => {
            trip.done(
                &state,
                Some(&caller),
                Some(&service),
                Some(&tool),
                response.status(),
                bytes_in,
                bytes_out,
                false,
            );
            response
        }
        Err(reject) => {
            let response = reject.into_response();
            trip.done(
                &state,
                Some(&caller),
                Some(&service),
                Some(&tool),
                response.status(),
                bytes_in,
                0,
                false,
            );
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(values: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(header::AUTHORIZATION, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn bearer_accepts_exactly_one_well_formed_token() {
        assert_eq!(bearer(&headers(&["Bearer abc"])), Some("abc"));
        assert_eq!(
            bearer(&headers(&["bearer abc"])),
            Some("abc"),
            "scheme is case-insensitive"
        );
    }

    #[test]
    fn bearer_rejects_everything_else() {
        assert_eq!(bearer(&HeaderMap::new()), None);
        for bad in [
            "Bearer",
            "Bearer ",
            "Basic abc",
            "abc",
            "Bearer a b",
            "Bearer  abc",
        ] {
            assert_eq!(bearer(&headers(&[bad])), None, "{bad:?}");
        }
        assert_eq!(bearer(&headers(&["Bearer a", "Bearer b"])), None, "two headers");
        assert_eq!(
            bearer(&headers(&[&format!("Bearer {}", "x".repeat(201))])),
            None,
            "too long"
        );
    }

    #[test]
    fn constant_time_compare_agrees_with_equality() {
        let a = sha256("one");
        let b = sha256("two");
        assert!(ct_eq(&a, &a));
        assert!(!ct_eq(&a, &b));
    }
}
