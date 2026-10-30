// Written by Kone Tshivhinda & Claude
//! The vault service's HTTP layer: `contracts/openapi/vault.v1.yaml`, nothing more.
//!
//! Every route except `/health` demands an internal token (ADR-0012) signed by the gateway and of
//! kind `principal`; the permission check (`settings:manage`) is a store rule, so it cannot be
//! bypassed by a route. Tenant, user and permissions come from the verified token, never from the
//! request.
//!
//! **No response carries a value.** The metadata type has no field for one, and the only plaintext
//! in the process is the [`LoadedSecret`] handed to the provider check, which is wiped on drop.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Query, Request, State};
use axum::http::{header, request::Parts, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use timelex_auth::{Caller, Verifier};
use timelex_vault::{SecretId, SecretKind};
use uuid::Uuid;

use crate::error::StoreError;
use crate::store::{
    Actor, LoadedSecret, PutOutcome, PutSecretRequest, SecretMetadata, TestOutcome, VaultStore,
};
use crate::validator::{CheckResult, ProviderValidator};

/// Largest request body accepted (also enforced at the gateway).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// What every handler needs.
#[derive(Clone)]
pub struct AppState {
    store: VaultStore,
    verifier: Arc<Verifier>,
    validator: Arc<dyn ProviderValidator>,
}

/// Build the router. Kept apart from `main` so tests can call it without a socket.
pub fn router(store: VaultStore, verifier: Verifier, validator: Arc<dyn ProviderValidator>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/secrets", get(list_secrets))
        // PUT names a kind and DELETE names an id, but they share one path segment. Axum will not
        // register the same segment under two parameter names, so the router sees `key` and each
        // handler interprets it: a kind for PUT, a UUID for DELETE.
        .route("/v1/secrets/{key}", put(put_secret).delete(revoke_secret))
        .route("/v1/secrets/{id}/test", post(test_secret))
        .route("/v1/secrets/{id}/rotate", post(rotate_secret))
        .fallback(|| async { ApiError::NotFound })
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(AppState {
            store,
            verifier: Arc::new(verifier),
            validator,
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
    /// The provider refused the credential (422).
    ProviderRejected,
    /// The provider was not reached, or its answer says nothing about the credential (502).
    ProviderUnreachable,
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
            ApiError::ProviderRejected => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "provider-rejected",
                "The provider rejected the secret",
                Some("the provider did not accept this credential".into()),
            ),
            ApiError::ProviderUnreachable => (
                StatusCode::BAD_GATEWAY,
                "provider-unreachable",
                "The provider could not be checked",
                Some("the provider did not answer; try again".into()),
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
            // The rule messages are fixed text about the shape of a value or host; they never
            // include the value itself.
            StoreError::Invalid(rule) => ApiError::BadRequest(rule.to_string()),
            StoreError::Forbidden => ApiError::Forbidden,
            StoreError::NotFound => ApiError::NotFound,
            StoreError::Revoked => ApiError::Conflict("the secret has been revoked"),
            StoreError::Internal(message) => {
                // Until telemetry exists (P1-10) the only record is stderr. Never log request bodies,
                // tokens or values.
                eprintln!("vault: internal error: {message}");
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
                eprintln!("vault: internal token rejected: {reason:?}");
                Err(ApiError::Unauthorized)
            }
        }
    }
}

impl Verified {
    /// Only a `principal` may touch a firm's secrets; the permission itself is checked by the store.
    fn require_principal(self) -> Result<Actor, ApiError> {
        match self.0 {
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

/// `Query<T>` with our error bodies, for the same reason as [`Body`].
struct Params<T>(T);

impl<T, S> FromRequestParts<S> for Params<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, ApiError> {
        Query::<T>::from_request_parts(parts, &())
            .await
            .map(|Query(v)| Params(v))
            .map_err(|_| ApiError::BadRequest("the query is not valid for this operation".into()))
    }
}

// ---------- wire types (the camelCase JSON of the contract) ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PutBody {
    value: String,
    label: Option<String>,
    #[serde(default)]
    allowed_hosts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RotateBody {
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    #[serde(default)]
    include_revoked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretMetadataJson {
    id: Uuid,
    kind: &'static str,
    label: String,
    last_four: String,
    allowed_hosts: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
    // The contract says these are absent until they happen, so they are omitted rather than null.
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    last_tested_at: Option<OffsetDateTime>,
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    last_used_at: Option<OffsetDateTime>,
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    revoked_at: Option<OffsetDateTime>,
}

impl From<&SecretMetadata> for SecretMetadataJson {
    fn from(m: &SecretMetadata) -> Self {
        Self {
            id: m.id.as_uuid(),
            kind: m.kind.as_str(),
            label: m.label.clone(),
            last_four: m.last_four.clone(),
            allowed_hosts: m.allowed_hosts.clone(),
            created_at: m.created_at,
            updated_at: m.updated_at,
            last_tested_at: m.last_tested_at,
            last_used_at: m.last_used_at,
            revoked_at: m.revoked_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretListJson {
    secrets: Vec<SecretMetadataJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestResultJson {
    ok: bool,
    #[serde(with = "time::serde::rfc3339")]
    checked_at: OffsetDateTime,
}

// ---------- helpers ----------

/// Nothing here is worth caching, and a cache between the vault and the gateway must never keep a
/// stale view of which key is live.
fn reply<T: IntoResponse>(status: StatusCode, body: T) -> Response {
    let mut r = (status, body).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn parse_id(raw: &str) -> Result<SecretId, ApiError> {
    Uuid::parse_str(raw)
        .map(SecretId::from_uuid)
        .map_err(|_| ApiError::BadRequest("id is not a UUID".into()))
}

// ---------- handlers ----------

async fn health(State(state): State<AppState>) -> Response {
    match state.store.ping().await {
        Ok(()) => reply(StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(_) => ApiError::Unavailable.into_response(),
    }
}

async fn list_secrets(
    State(state): State<AppState>,
    caller: Verified,
    Params(query): Params<ListQuery>,
) -> Result<Response, ApiError> {
    let actor = caller.require_principal()?;
    let secrets = state.store.list(actor, query.include_revoked).await?;
    Ok(reply(
        StatusCode::OK,
        Json(SecretListJson {
            secrets: secrets.iter().map(SecretMetadataJson::from).collect(),
        }),
    ))
}

async fn put_secret(
    State(state): State<AppState>,
    caller: Verified,
    Path(kind): Path<String>,
    Body(body): Body<PutBody>,
) -> Result<Response, ApiError> {
    let actor = caller.require_principal()?;
    let kind = SecretKind::parse(&kind)
        .ok_or_else(|| ApiError::BadRequest("kind is not a known secret kind".into()))?;
    let label = body.label.unwrap_or_else(|| kind.display_name().to_owned());
    let outcome = state
        .store
        .put(
            actor,
            kind,
            PutSecretRequest {
                label,
                value: body.value,
                allowed_hosts: body.allowed_hosts,
            },
        )
        .await?;
    Ok(match outcome {
        PutOutcome::Created(m) => reply(StatusCode::CREATED, Json(SecretMetadataJson::from(&m))),
        PutOutcome::Replaced(m) => reply(StatusCode::OK, Json(SecretMetadataJson::from(&m))),
    })
}

async fn test_secret(
    State(state): State<AppState>,
    caller: Verified,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = caller.require_principal()?;
    let id = parse_id(&id)?;
    // The value is opened here, checked, and wiped when `secret` drops at the end of this function.
    let secret: LoadedSecret = state.store.open_for_test(actor.clone(), id).await?;
    match state.validator.check(&secret).await {
        CheckResult::Accepted => {
            let checked_at = state.store.record_test(actor, id, TestOutcome::Passed).await?;
            Ok(reply(
                StatusCode::OK,
                Json(TestResultJson { ok: true, checked_at }),
            ))
        }
        CheckResult::Rejected => {
            // A rejection is a verdict about the credential, so it is recorded.
            state.store.record_test(actor, id, TestOutcome::Failed).await?;
            Err(ApiError::ProviderRejected)
        }
        // No verdict was reached, so `lastTestedAt` is left alone: a provider outage must not look
        // like a bad key and prompt an admin to revoke one that works.
        CheckResult::Unreachable => Err(ApiError::ProviderUnreachable),
    }
}

async fn rotate_secret(
    State(state): State<AppState>,
    caller: Verified,
    Path(id): Path<String>,
    Body(body): Body<RotateBody>,
) -> Result<Response, ApiError> {
    let actor = caller.require_principal()?;
    let id = parse_id(&id)?;
    let metadata = state.store.rotate(actor, id, body.value).await?;
    Ok(reply(StatusCode::OK, Json(SecretMetadataJson::from(&metadata))))
}

async fn revoke_secret(
    State(state): State<AppState>,
    caller: Verified,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = caller.require_principal()?;
    let id = parse_id(&id)?;
    state.store.revoke(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
