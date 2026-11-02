// Written by Kone Tshivhinda & Claude
//! The capture-hub HTTP layer: `contracts/openapi/capture.v1.yaml`.
//!
//! `POST /v1/events` is the one route of this increment. It takes one activity event from a
//! principal with `capture:write` and stores it idempotently: the first delivery answers `201`, a
//! duplicate answers `200` with `created: false`. Every other route will arrive with the connector
//! that needs it.
//!
//! The firm and the user come from the verified internal token (ADR-0012), never from the request.
//! The body's `tenantId` and `actor.userId` are checked against them and then rewritten to the one
//! canonical spelling, so the stored event names its firm and user exactly once.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Request, State};
use axum::http::{header, request::Parts, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use timelex_auth::{Caller, Principal, Verifier};
use timelex_types::{supports_activity_schema_version, ActivityEvent};
use uuid::Uuid;

use crate::error::StoreError;
use crate::exclusions::{Exclusion, ExclusionKind};
use crate::manual::{self, Entry};
use crate::store::{Actor, CaptureStore, Clock, IngestOutcome};

/// Largest request body accepted (also enforced at the gateway).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// What every handler needs.
#[derive(Clone)]
pub struct AppState {
    store: CaptureStore,
    verifier: Arc<Verifier>,
    clock: Clock,
}

/// Build the router. Kept apart from `main` so tests can call it without a socket.
pub fn router(store: CaptureStore, verifier: Verifier) -> Router {
    router_with_clock(store, verifier, Arc::new(OffsetDateTime::now_utc))
}

/// The router with an injected clock, so a manual entry's `capturedAt` is testable.
pub fn router_with_clock(store: CaptureStore, verifier: Verifier, clock: Clock) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/v1/events", post(ingest_event))
        .route("/v1/entries", post(capture_entry))
        .route("/v1/exclusions", get(list_exclusions).post(add_exclusion))
        .route("/v1/exclusions/{kind}/{value}", delete(remove_exclusion))
        .fallback(|| async { ApiError::NotFound })
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(AppState {
            store,
            verifier: Arc::new(verifier),
            clock,
        });
    timelex_telemetry::instrument(router, "capture")
}

// ---------- errors: RFC 9457 problem details ----------

/// Every failure the HTTP layer can report. Bodies never contain internal details.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    BadRequest(String),
    NotFound,
    TooLarge,
    Unavailable,
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, slug, title, detail): (StatusCode, &str, &str, Option<String>) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthenticated", "Unauthorized", None),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Forbidden", None),
            ApiError::BadRequest(detail) => (
                StatusCode::BAD_REQUEST,
                "invalid-request",
                "Invalid request",
                Some(detail),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not-found", "Not found", None),
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
        if let Some(detail) = detail {
            body["detail"] = detail.into();
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
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Invalid(message) => ApiError::BadRequest(message.to_owned()),
            StoreError::Forbidden => ApiError::Forbidden,
            StoreError::Internal(message) => {
                tracing::error!(error = %message, "store failure");
                ApiError::Internal
            }
        }
    }
}

// ---------- authentication of the caller (the gateway) ----------

/// A verified internal token. Extracting it is what "this route needs a caller" means.
struct Verified(Principal);

impl FromRequestParts<AppState> for Verified {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match state.verifier.verify(token) {
            // Only a tenant principal submits activity. Service and operator tokens are refused.
            Ok(verified) => match verified.caller {
                Caller::Principal(principal) => Ok(Verified(principal)),
                _ => Err(ApiError::Forbidden),
            },
            Err(reason) => {
                // The reason is for the operator; the caller always gets the same 401.
                tracing::warn!(?reason, "internal token rejected");
                Err(ApiError::Unauthorized)
            }
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
            Ok(Json(value)) => Ok(Body(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => Err(ApiError::TooLarge),
            Err(_) => Err(ApiError::BadRequest(
                "the request body is not valid for this operation".into(),
            )),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestJson {
    id: String,
    created: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewExclusion {
    kind: String,
    value: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Serialize)]
struct ExclusionsJson {
    exclusions: Vec<Exclusion>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExclusionJson {
    kind: ExclusionKind,
    value: String,
    created: bool,
}

// ---------- handlers ----------

async fn health(State(state): State<AppState>) -> Response {
    match state.store.ping().await {
        Ok(()) => reply(StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(_) => ApiError::Unavailable.into_response(),
    }
}

/// `POST /v1/events`: store one activity event for the caller's firm.
async fn ingest_event(
    State(state): State<AppState>,
    caller: Verified,
    Body(mut event): Body<ActivityEvent>,
) -> Result<Response, ApiError> {
    if !supports_activity_schema_version(&event.schema_version) {
        return Err(ApiError::BadRequest(
            "schemaVersion is not a supported 1.x activity-event version".into(),
        ));
    }
    if !same_id(&event.tenant_id, "tn_", caller.0.tenant.as_uuid()) {
        return Err(ApiError::BadRequest(
            "the event belongs to a different tenant".into(),
        ));
    }
    if !same_id(&event.actor.user_id, "us_", caller.0.user.as_uuid()) {
        return Err(ApiError::Forbidden);
    }
    // One spelling is stored, whichever spelling the caller used.
    event.tenant_id = format!("tn_{}", caller.0.tenant.to_hex32());
    event.actor.user_id = format!("us_{}", caller.0.user.as_uuid().simple());

    store_and_reply(&state, Actor::from(&caller.0), event).await
}

/// `POST /v1/entries`: a person records work no connector can see.
async fn capture_entry(
    State(state): State<AppState>,
    caller: Verified,
    Body(entry): Body<Entry>,
) -> Result<Response, ApiError> {
    let event = manual::build_event(caller.0.tenant, caller.0.user, entry, (state.clock)())?;
    store_and_reply(&state, Actor::from(&caller.0), event).await
}

/// `GET /v1/exclusions`: the firm's privacy exclusions.
async fn list_exclusions(State(state): State<AppState>, caller: Verified) -> Result<Response, ApiError> {
    let exclusions = state.store.list_exclusions(Actor::from(&caller.0)).await?;
    Ok(reply(StatusCode::OK, Json(ExclusionsJson { exclusions })))
}

/// `POST /v1/exclusions`: configure one exclusion. A repeat is 200 with `created: false`.
async fn add_exclusion(
    State(state): State<AppState>,
    caller: Verified,
    Body(new): Body<NewExclusion>,
) -> Result<Response, ApiError> {
    let kind = ExclusionKind::parse(&new.kind)?;
    let (exclusion, created) = state
        .store
        .add_exclusion(Actor::from(&caller.0), kind, &new.value, new.reason.as_deref())
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok(reply(
        status,
        Json(ExclusionJson {
            kind: exclusion.kind,
            value: exclusion.value,
            created,
        }),
    ))
}

/// `DELETE /v1/exclusions/{kind}/{value}`: stop excluding. 204 when removed, 404 when absent.
async fn remove_exclusion(
    State(state): State<AppState>,
    caller: Verified,
    Path((kind, value)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let kind = ExclusionKind::parse(&kind)?;
    if state
        .store
        .remove_exclusion(Actor::from(&caller.0), kind, &value)
        .await?
    {
        Ok(reply(StatusCode::NO_CONTENT, ()))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Store one event for a user and answer with what the store decided. The first delivery is 201;
/// a duplicate is 200 with `created: false`.
async fn store_and_reply(state: &AppState, actor: Actor, event: ActivityEvent) -> Result<Response, ApiError> {
    let outcome = state.store.ingest(actor, event).await?;
    let status = match outcome.outcome {
        IngestOutcome::Created => StatusCode::CREATED,
        // A duplicate and a signal the firm excludes both store nothing; the caller may retry
        // either, because an excluded signal is not a duplicate and its answer stays honest.
        IngestOutcome::Existing | IngestOutcome::Excluded => StatusCode::OK,
    };
    Ok(reply(
        status,
        Json(IngestJson {
            id: outcome.id,
            created: outcome.outcome == IngestOutcome::Created,
        }),
    ))
}

/// Whether `value` is `prefix` followed by the UUID in any accepted spelling.
fn same_id(value: &str, prefix: &str, expected: Uuid) -> bool {
    value
        .strip_prefix(prefix)
        .and_then(|rest| Uuid::parse_str(rest).ok())
        .is_some_and(|id| id == expected)
}

/// Nothing here is worth caching: a duplicate delivery is answered from the store, not a cache.
fn reply<T: IntoResponse>(status: StatusCode, body: T) -> Response {
    let mut response = (status, body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
