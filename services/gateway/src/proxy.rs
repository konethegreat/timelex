// Written by Kone Tshivhinda & Claude
//! Authentication and routing: `contracts/openapi/gateway.v1.yaml`, ADR-0012.
//!
//! The gateway never forwards what a client sent. It checks the caller, then builds a *new* request
//! to the service carrying only an internal token and the body. So no header a client chose (an
//! `X-TimeLex-Tenant`, a cookie, the API key itself) can reach a service.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use timelex_auth::{Caller, Principal};
use timelex_identity::{ApiKeyId, Permission, TenantId, UserId};
use uuid::Uuid;

use crate::config::{sha256, Config};

/// Largest request body the gateway will read and forward.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

struct Inner {
    config: Config,
    http: reqwest::Client,
}

/// Shared gateway state: configuration and one pooled HTTP client.
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
        Ok(Self(Arc::new(Inner { config, http })))
    }
}

/// The routes this version forwards, and who may use each. `authenticate` is deliberately absent.
pub fn routes(state: GatewayState) -> Router {
    Router::new()
        .route("/v1/tenants", post(create_tenant))
        .route("/v1/api-keys", post(issue_api_key))
        .route("/v1/api-keys/{key_id}/revoke", post(revoke_api_key))
        .with_state(state)
}

// ---------- problem details ----------

/// Why the gateway answered itself instead of relaying a service's answer.
enum Reject {
    Unauthorized,
    Invalid(&'static str),
    TooLarge,
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
            Reject::TooLarge => problem(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too-large",
                "Request too large",
                None,
            ),
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

/// The one bearer token in the request, or `None` for anything else (missing, repeated, another
/// scheme, empty, absurdly long). Callers answer `None` with the same 401 as a wrong key.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let mut all = headers.get_all(header::AUTHORIZATION).iter();
    let (first, rest) = (all.next()?, all.next());
    if rest.is_some() {
        return None;
    }
    let (scheme, token) = first.to_str().ok()?.split_once(' ')?;
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
        let token = self
            .0
            .config
            .signer
            .sign("identity", &Caller::Service)
            .map_err(|_| Reject::Internal)?;
        let response = self
            .0
            .http
            .post(format!("{}/v1/authenticate", self.0.config.identity_url))
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

    /// Identify the caller and mint the internal token to forward with.
    async fn internal_token(&self, who: Who, headers: &HeaderMap) -> Result<String, Reject> {
        let presented = bearer(headers).ok_or(Reject::Unauthorized)?;
        let caller = match who {
            Who::Operator => {
                if !self.is_operator(presented) {
                    return Err(Reject::Unauthorized);
                }
                Caller::Operator
            }
            Who::Tenant => Caller::Principal(self.authenticate(presented).await?),
        };
        self.0
            .config
            .signer
            .sign("identity", &caller)
            .map_err(|_| Reject::Internal)
    }

    /// Send a fresh request to identity and relay the answer.
    async fn forward(
        &self,
        path: &str,
        token: String,
        body: Option<axum::body::Bytes>,
    ) -> Result<Response, Reject> {
        let mut request = self
            .0
            .http
            .post(format!("{}{path}", self.0.config.identity_url))
            .bearer_auth(token);
        if let Some(b) = body.filter(|b| !b.is_empty()) {
            request = request.header(header::CONTENT_TYPE, "application/json").body(b);
        }
        let upstream = request.send().await.map_err(|e| upstream_failure(&e))?;
        let status = upstream.status();
        // A 401 here means identity refused *our* token (misconfiguration), and a 5xx is its own
        // failure; neither is the client's business, so neither is relayed.
        if status == StatusCode::UNAUTHORIZED || status.is_server_error() {
            return Err(Reject::BadGateway);
        }
        let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
        let cache_control = upstream.headers().get(header::CACHE_CONTROL).cloned();
        let bytes = upstream.bytes().await.map_err(|e| upstream_failure(&e))?;
        let mut response = Response::new(Body::from(bytes));
        *response.status_mut() = status;
        if let Some(v) = content_type {
            response.headers_mut().insert(header::CONTENT_TYPE, v);
        }
        if let Some(v) = cache_control {
            response.headers_mut().insert(header::CACHE_CONTROL, v);
        }
        Ok(response)
    }
}

#[derive(Clone, Copy)]
enum Who {
    Operator,
    Tenant,
}

/// Authenticate, then read the (size-limited) body, then forward. Authentication comes first so an
/// unauthenticated caller learns nothing about body limits or routes.
async fn handle(state: GatewayState, who: Who, path: String, req: Request) -> Result<Response, Reject> {
    let token = state.internal_token(who, req.headers()).await?;
    let body = to_bytes(req.into_body(), MAX_BODY_BYTES)
        .await
        .map_err(|_| Reject::TooLarge)?;
    state.forward(&path, token, Some(body)).await
}

fn done(r: Result<Response, Reject>) -> Response {
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn create_tenant(State(s): State<GatewayState>, req: Request) -> Response {
    done(handle(s, Who::Operator, "/v1/tenants".into(), req).await)
}

async fn issue_api_key(State(s): State<GatewayState>, req: Request) -> Response {
    done(handle(s, Who::Tenant, "/v1/api-keys".into(), req).await)
}

async fn revoke_api_key(State(s): State<GatewayState>, Path(key_id): Path<String>, req: Request) -> Response {
    // Re-format the id ourselves: only a canonical UUID ever reaches the service's URL.
    let Ok(key) = Uuid::parse_str(&key_id) else {
        // Authenticate first even for a bad id, so the answer never depends on who is asking.
        return done(match s.internal_token(Who::Tenant, req.headers()).await {
            Ok(_) => Err(Reject::Invalid("keyId is not a UUID")),
            Err(e) => Err(e),
        });
    };
    done(
        handle(
            s,
            Who::Tenant,
            format!("/v1/api-keys/{}/revoke", key.hyphenated()),
            req,
        )
        .await,
    )
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
