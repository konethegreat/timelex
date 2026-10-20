// Written by Kone Tshivhinda & Claude
//! The identity service's HTTP layer: `contracts/openapi/identity.v1.yaml`, nothing more.
//!
//! Every route except `/health` demands an internal token (ADR-0012) signed by the gateway and checks
//! that its *kind* is the one the route accepts. Tenant, user and permissions come from the verified
//! token, never from the request. No tenant API key is ever accepted here.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Request, State};
use axum::http::{header, request::Parts, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use timelex_auth::{Caller, Kind, Verifier};
use timelex_identity::{ApiKeyId, IdentityError, NewTenantRequest, Permission, UserId};
use uuid::Uuid;

use crate::error::{AuthError, StoreError};
use crate::store::{Actor, CreatedTenant, IdentityStore, IssueKeyRequest, IssuedKey, Principal};

/// Largest request body accepted (also enforced at the gateway).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// What every handler needs.
#[derive(Clone)]
pub struct AppState {
    store: IdentityStore,
    verifier: Arc<Verifier>,
}

/// Build the router. Kept apart from `main` so tests can call it without a socket.
pub fn router(store: IdentityStore, verifier: Verifier) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/tenants", post(create_tenant))
        .route("/v1/api-keys", post(issue_api_key))
        .route("/v1/api-keys/{key_id}/revoke", post(revoke_api_key))
        .route("/v1/authenticate", post(authenticate))
        .fallback(|| async { ApiError::NotFound })
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(AppState {
            store,
            verifier: Arc::new(verifier),
        })
}

// ---------- errors: RFC 9457 problem details ----------

/// Every failure the HTTP layer can report. Bodies never contain secrets or internal details.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    BadRequest(String),
    NotFound,
    Conflict(&'static str),
    TooLarge,
    Unavailable,
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, slug, title, detail): (StatusCode, &str, &str, Option<String>) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthenticated", "Unauthorized", None),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Forbidden", None),
            ApiError::BadRequest(d) => (
                StatusCode::BAD_REQUEST,
                "invalid-request",
                "Invalid request",
                Some(d),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not-found", "Not found", None),
            ApiError::Conflict(d) => (StatusCode::CONFLICT, "conflict", "Conflict", Some(d.to_owned())),
            ApiError::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "too-large",
                "Request too large",
                None,
            ),
            ApiError::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Service unavailable",
                None,
            ),
            ApiError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal error",
                None,
            ),
        };
        let mut body = json!({
            "type": format!("urn:timelex:problem:{slug}"),
            "title": title,
            "status": status.as_u16(),
        });
        if let Some(d) = detail {
            body["detail"] = d.into();
        }
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Rule(IdentityError::PermissionDenied(_)) => ApiError::Forbidden,
            StoreError::Rule(rule) => ApiError::BadRequest(rule.to_string()),
            StoreError::NotFound => ApiError::NotFound,
            StoreError::SlugTaken => ApiError::Conflict("the slug is already taken"),
            StoreError::Internal(message) => {
                // Until telemetry exists (P1-10) the only record is stderr. Never log request bodies or tokens.
                eprintln!("identity: internal error: {message}");
                ApiError::Internal
            }
        }
    }
}

// ---------- authentication of the *caller* (the gateway) ----------

/// A verified internal token. Extracting it is what "this route needs a caller" means.
struct Verified(Caller);

impl FromRequestParts<AppState> for Verified {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match state.verifier.verify(token) {
            Ok(v) => Ok(Verified(v.caller)),
            Err(reason) => {
                // The reason is for the operator; the caller always gets the same 401.
                eprintln!("identity: internal token rejected: {reason:?}");
                Err(ApiError::Unauthorized)
            }
        }
    }
}

impl Verified {
    /// Require a specific kind of caller; a valid token of another kind is a 403.
    fn require(self, kind: Kind) -> Result<Caller, ApiError> {
        if self.0.kind() == kind {
            Ok(self.0)
        } else {
            Err(ApiError::Forbidden)
        }
    }

    fn require_principal(self) -> Result<Actor, ApiError> {
        match self.require(Kind::Principal)? {
            Caller::Principal(p) => Ok(Actor::from(&p)),
            _ => Err(ApiError::Forbidden),
        }
    }
}

/// `Json<T>` with our error bodies. The detail is fixed text: serde's own message can echo values.
struct Body<T>(T);

impl<T, S> FromRequest<S> for Body<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(Body(v)),
            Err(r) if r.status() == StatusCode::PAYLOAD_TOO_LARGE => Err(ApiError::TooLarge),
            Err(_) => Err(ApiError::BadRequest(
                "the request body is not valid for this operation".into(),
            )),
        }
    }
}

// ---------- wire types (the camelCase JSON of the contract) ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTenantBody {
    slug: String,
    display_name: String,
    admin_email: String,
    admin_display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IssueKeyBody {
    user_id: Option<Uuid>,
    name: String,
    scopes: Vec<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    expires_at: Option<OffsetDateTime>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticateBody {
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IssuedKeyJson {
    id: Uuid,
    user_id: Uuid,
    name: String,
    token: String,
    prefix: String,
    scopes: Vec<&'static str>,
    #[serde(with = "time::serde::rfc3339::option")]
    expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

impl From<&IssuedKey> for IssuedKeyJson {
    fn from(k: &IssuedKey) -> Self {
        Self {
            id: k.id.as_uuid(),
            user_id: k.user_id.as_uuid(),
            name: k.name.clone(),
            token: k.token.expose_secret().to_owned(),
            prefix: k.token.display_prefix().to_owned(),
            scopes: k.scopes.iter().map(Permission::as_str).collect(),
            expires_at: k.expires_at,
            created_at: k.created_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTenantJson {
    tenant_id: Uuid,
    slug: String,
    admin_user_id: Uuid,
    initial_api_key: IssuedKeyJson,
}

impl From<&CreatedTenant> for CreateTenantJson {
    fn from(c: &CreatedTenant) -> Self {
        Self {
            tenant_id: c.tenant_id.as_uuid(),
            slug: c.slug.as_str().to_owned(),
            admin_user_id: c.admin_user_id.as_uuid(),
            initial_api_key: IssuedKeyJson::from(&c.initial_key),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrincipalJson {
    tenant_id: Uuid,
    user_id: Uuid,
    key_id: Uuid,
    roles: Vec<&'static str>,
    permissions: Vec<&'static str>,
}

impl From<&Principal> for PrincipalJson {
    fn from(p: &Principal) -> Self {
        Self {
            tenant_id: p.tenant.as_uuid(),
            user_id: p.user.as_uuid(),
            key_id: p.key.as_uuid(),
            roles: p.roles.iter().map(|r| r.as_str()).collect(),
            permissions: p.permissions.iter().map(Permission::as_str).collect(),
        }
    }
}

/// A response carrying a secret must never be cached by anything between us and the caller.
fn no_store<T: IntoResponse>(status: StatusCode, body: T) -> Response {
    let mut r = (status, body).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

// ---------- handlers ----------

async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.store.ping().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn create_tenant(
    State(state): State<AppState>,
    caller: Verified,
    Body(b): Body<CreateTenantBody>,
) -> Result<Response, ApiError> {
    caller.require(Kind::Operator)?;
    let created = state
        .store
        .create_tenant(&NewTenantRequest {
            slug: &b.slug,
            display_name: &b.display_name,
            admin_email: &b.admin_email,
            admin_display_name: &b.admin_display_name,
        })
        .await?;
    Ok(no_store(
        StatusCode::CREATED,
        Json(CreateTenantJson::from(&created)),
    ))
}

async fn issue_api_key(
    State(state): State<AppState>,
    caller: Verified,
    Body(b): Body<IssueKeyBody>,
) -> Result<Response, ApiError> {
    let actor = caller.require_principal()?;
    let scopes = b
        .scopes
        .iter()
        .map(|s| Permission::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ApiError::BadRequest("a scope is not a known permission".into()))?;
    let issued = state
        .store
        .issue_api_key(
            actor,
            IssueKeyRequest {
                owner: b.user_id.map(UserId::from_uuid),
                name: b.name,
                scopes,
                expires_at: b.expires_at,
            },
        )
        .await?;
    Ok(no_store(StatusCode::CREATED, Json(IssuedKeyJson::from(&issued))))
}

async fn revoke_api_key(
    State(state): State<AppState>,
    caller: Verified,
    Path(key_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = caller.require_principal()?;
    let key = Uuid::parse_str(&key_id).map_err(|_| ApiError::BadRequest("keyId is not a UUID".into()))?;
    state
        .store
        .revoke_api_key(actor, ApiKeyId::from_uuid(key))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn authenticate(
    State(state): State<AppState>,
    caller: Verified,
    Body(b): Body<AuthenticateBody>,
) -> Result<Response, ApiError> {
    caller.require(Kind::Service)?;
    match state.store.authenticate(&b.token).await {
        Ok(p) => Ok(no_store(StatusCode::OK, Json(PrincipalJson::from(&p)))),
        Err(AuthError::Internal(message)) => {
            eprintln!("identity: internal error during authenticate: {message}");
            Err(ApiError::Internal)
        }
        Err(reason) => {
            // Every cause is the same 401 for the caller; the cause goes to the operator's log.
            eprintln!("identity: authenticate refused: {reason}");
            Err(ApiError::Unauthorized)
        }
    }
}
