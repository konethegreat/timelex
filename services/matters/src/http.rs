// Written by Kone Tshivhinda & Claude
//! The matters service's HTTP layer: `contracts/openapi/matters.v1.yaml`, ADR-0027.
//!
//! Every route except `/health` demands an internal token signed by the gateway (ADR-0012) and
//! accepts only a tenant principal. Reads need `time:read`; writes and imports need `settings:manage`.
//! The firm comes from the verified token, never from the request. The service listens on a private
//! interface; the gateway fronts it in every other environment.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Query, Request, State};
use axum::http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use timelex_auth::{Caller, Kind, Verifier};
use timelex_identity::Permission;
use uuid::Uuid;

use crate::domain::{AliasKind, ClientStatus, MatterStatus, Rounding};
use crate::error::StoreError;
use crate::import::{self, ImportError, ImportKind, Rows};
use crate::store::{
    Actor, Alias, Client, Matter, NewAlias, NewClient, NewMatter, NewRateCard, RateCard, Store,
};

/// Largest JSON body accepted (also enforced at the gateway).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Largest CSV body accepted. 5 000 rows fit comfortably; the row count is the tighter limit.
pub const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;

/// What every handler needs.
#[derive(Clone)]
pub struct AppState {
    store: Store,
    verifier: Arc<Verifier>,
}

/// Build the router. Kept apart from `main` so tests can call it without a socket.
pub fn router(store: Store, verifier: Verifier) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/v1/clients", get(list_clients).post(create_client))
        .route("/v1/matters", get(list_matters).post(create_matter))
        .route("/v1/matters/{matter_id}/rate-card", put(put_rate_card))
        .route("/v1/rate-cards", get(list_rate_cards))
        .route("/v1/aliases", get(list_aliases).post(create_alias))
        .route(
            "/v1/import/{kind}",
            post(import_csv).layer(DefaultBodyLimit::max(MAX_IMPORT_BYTES)),
        )
        .fallback(|| async { ApiError::NotFound })
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(AppState {
            store,
            verifier: Arc::new(verifier),
        });
    timelex_telemetry::instrument(router, "matters")
}

// ---------- errors: RFC 9457 problem details ----------

/// Every failure the HTTP layer can report. Bodies never contain secrets or internal details.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    BadRequest(String),
    NotFound,
    Conflict(String),
    UnsupportedMediaType,
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
            ApiError::Conflict(detail) => (StatusCode::CONFLICT, "conflict", "Conflict", Some(detail)),
            ApiError::UnsupportedMediaType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported-media-type",
                "Unsupported media type",
                Some("the import body must be text/csv".into()),
            ),
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
            StoreError::NotFound => ApiError::NotFound,
            StoreError::Conflict(message) => ApiError::Conflict(message.to_owned()),
            StoreError::Row { row, message } => ApiError::BadRequest(format!("row {row}: {message}")),
            StoreError::Internal(message) => {
                tracing::error!(error = %message, "store failure");
                ApiError::Internal
            }
        }
    }
}

impl From<ImportError> for ApiError {
    fn from(error: ImportError) -> Self {
        ApiError::BadRequest(error.to_string())
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
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match state.verifier.verify(token) {
            Ok(verified) => Ok(Verified(verified.caller)),
            Err(reason) => {
                // The reason is for the operator; the caller always gets the same 401.
                tracing::warn!(?reason, "internal token rejected");
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

    /// Require a tenant principal that holds `permission`.
    fn require_principal(self, permission: Permission) -> Result<Actor, ApiError> {
        match self.require(Kind::Principal)? {
            Caller::Principal(principal) => {
                if principal.permissions.contains(&permission) {
                    Ok(Actor::from(&principal))
                } else {
                    Err(ApiError::Forbidden)
                }
            }
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
            Ok(Json(value)) => Ok(Body(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => Err(ApiError::TooLarge),
            Err(_) => Err(ApiError::BadRequest(
                "the request body is not valid for this operation".into(),
            )),
        }
    }
}

/// A UTF-8 import body, read up to [`MAX_IMPORT_BYTES`].
struct CsvBody(String);

impl<S> FromRequest<S> for CsvBody
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, _state: &S) -> Result<Self, ApiError> {
        let bytes = to_bytes(req.into_body(), MAX_IMPORT_BYTES)
            .await
            .map_err(|_| ApiError::TooLarge)?;
        String::from_utf8(bytes.to_vec())
            .map(CsvBody)
            .map_err(|_| ApiError::BadRequest("the import body must be UTF-8 text".into()))
    }
}

// ---------- wire types (camelCase JSON) ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateClientBody {
    client_id: String,
    name: String,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateMatterBody {
    matter_id: String,
    client_id: String,
    reference: String,
    title: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    practice_area: Option<String>,
    #[serde(default)]
    opened_on: Option<String>,
    #[serde(default)]
    closed_on: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateAliasBody {
    kind: String,
    value: String,
    client_id: String,
    #[serde(default)]
    matter_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RateCardBody {
    hourly_rate_cents: i64,
    #[serde(default)]
    unit_minutes: Option<i64>,
    #[serde(default)]
    rounding: Option<String>,
    #[serde(default)]
    minimum_units: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListMattersQuery {
    #[serde(default)]
    client_id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListAliasesQuery {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientJson {
    client_id: String,
    name: String,
    status: ClientStatus,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl From<&Client> for ClientJson {
    fn from(client: &Client) -> Self {
        Self {
            client_id: client.client_id.clone(),
            name: client.name.clone(),
            status: client.status,
            created_at: client.created_at,
            updated_at: client.updated_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatterJson {
    matter_id: String,
    client_id: String,
    reference: String,
    title: String,
    status: MatterStatus,
    practice_area: Option<String>,
    opened_on: Option<String>,
    closed_on: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl From<&Matter> for MatterJson {
    fn from(matter: &Matter) -> Self {
        Self {
            matter_id: matter.matter_id.clone(),
            client_id: matter.client_id.clone(),
            reference: matter.reference.clone(),
            title: matter.title.clone(),
            status: matter.status,
            practice_area: matter.practice_area.clone(),
            opened_on: matter.opened_on.map(|date| date.to_string()),
            closed_on: matter.closed_on.map(|date| date.to_string()),
            created_at: matter.created_at,
            updated_at: matter.updated_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AliasJson {
    id: Uuid,
    kind: AliasKind,
    value: String,
    client_id: String,
    matter_id: Option<String>,
}

impl From<&Alias> for AliasJson {
    fn from(alias: &Alias) -> Self {
        Self {
            id: alias.id,
            kind: alias.kind,
            value: alias.value.clone(),
            client_id: alias.client_id.clone(),
            matter_id: alias.matter_id.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RateCardJson {
    matter_id: String,
    hourly_rate_cents: i64,
    unit_minutes: i64,
    rounding: Rounding,
    minimum_units: i64,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl From<&RateCard> for RateCardJson {
    fn from(card: &RateCard) -> Self {
        Self {
            matter_id: card.matter_id.clone(),
            hourly_rate_cents: card.hourly_rate_cents,
            unit_minutes: card.unit_minutes,
            rounding: card.rounding,
            minimum_units: card.minimum_units,
            updated_at: card.updated_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientsJson {
    clients: Vec<ClientJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MattersJson {
    matters: Vec<MatterJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AliasesJson {
    aliases: Vec<AliasJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RateCardsJson {
    rate_cards: Vec<RateCardJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportJson {
    kind: &'static str,
    rows: usize,
}

// ---------- handlers ----------

async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.store.ping().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn list_clients(
    State(state): State<AppState>,
    caller: Verified,
) -> Result<Json<ClientsJson>, ApiError> {
    let actor = caller.require_principal(Permission::TimeRead)?;
    let clients = state.store.list_clients(actor).await?;
    Ok(Json(ClientsJson {
        clients: clients.iter().map(ClientJson::from).collect(),
    }))
}

async fn create_client(
    State(state): State<AppState>,
    caller: Verified,
    Body(body): Body<CreateClientBody>,
) -> Result<(StatusCode, Json<ClientJson>), ApiError> {
    let actor = caller.require_principal(Permission::SettingsManage)?;
    let client = state
        .store
        .create_client(
            actor,
            NewClient {
                client_id: body.client_id,
                name: body.name,
                status: body.status,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(ClientJson::from(&client))))
}

async fn list_matters(
    State(state): State<AppState>,
    caller: Verified,
    query: Result<Query<ListMattersQuery>, QueryRejection>,
) -> Result<Json<MattersJson>, ApiError> {
    let actor = caller.require_principal(Permission::TimeRead)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest("the query string is not valid".into()))?;
    let matters = state
        .store
        .list_matters(actor, query.client_id.as_deref())
        .await?;
    Ok(Json(MattersJson {
        matters: matters.iter().map(MatterJson::from).collect(),
    }))
}

async fn create_matter(
    State(state): State<AppState>,
    caller: Verified,
    Body(body): Body<CreateMatterBody>,
) -> Result<(StatusCode, Json<MatterJson>), ApiError> {
    let actor = caller.require_principal(Permission::SettingsManage)?;
    let matter = state
        .store
        .create_matter(
            actor,
            NewMatter {
                matter_id: body.matter_id,
                client_id: body.client_id,
                reference: body.reference,
                title: body.title,
                status: body.status,
                practice_area: body.practice_area,
                opened_on: body.opened_on,
                closed_on: body.closed_on,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(MatterJson::from(&matter))))
}

async fn put_rate_card(
    State(state): State<AppState>,
    caller: Verified,
    Path(matter_id): Path<String>,
    Body(body): Body<RateCardBody>,
) -> Result<Json<RateCardJson>, ApiError> {
    let actor = caller.require_principal(Permission::SettingsManage)?;
    let card = state
        .store
        .put_rate_card(
            actor,
            NewRateCard {
                matter_id,
                hourly_rate_cents: body.hourly_rate_cents,
                unit_minutes: body.unit_minutes,
                rounding: body.rounding,
                minimum_units: body.minimum_units,
            },
        )
        .await?;
    Ok(Json(RateCardJson::from(&card)))
}

async fn list_rate_cards(
    State(state): State<AppState>,
    caller: Verified,
) -> Result<Json<RateCardsJson>, ApiError> {
    let actor = caller.require_principal(Permission::TimeRead)?;
    let cards = state.store.list_rate_cards(actor).await?;
    Ok(Json(RateCardsJson {
        rate_cards: cards.iter().map(RateCardJson::from).collect(),
    }))
}

async fn list_aliases(
    State(state): State<AppState>,
    caller: Verified,
    query: Result<Query<ListAliasesQuery>, QueryRejection>,
) -> Result<Json<AliasesJson>, ApiError> {
    let actor = caller.require_principal(Permission::TimeRead)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest("the query string is not valid".into()))?;
    let kind = match query.kind.as_deref() {
        Some(raw) => Some(AliasKind::parse(raw).map_err(|message| ApiError::BadRequest(message.into()))?),
        None => None,
    };
    let aliases = state
        .store
        .list_aliases(actor, kind, query.value.as_deref())
        .await?;
    Ok(Json(AliasesJson {
        aliases: aliases.iter().map(AliasJson::from).collect(),
    }))
}

async fn create_alias(
    State(state): State<AppState>,
    caller: Verified,
    Body(body): Body<CreateAliasBody>,
) -> Result<(StatusCode, Json<AliasJson>), ApiError> {
    let actor = caller.require_principal(Permission::SettingsManage)?;
    let (alias, created) = state
        .store
        .create_alias(
            actor,
            NewAlias {
                kind: body.kind,
                value: body.value,
                client_id: body.client_id,
                matter_id: body.matter_id,
            },
        )
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(AliasJson::from(&alias))))
}

async fn import_csv(
    State(state): State<AppState>,
    caller: Verified,
    Path(kind): Path<String>,
    headers: HeaderMap,
    CsvBody(body): CsvBody,
) -> Result<Json<ImportJson>, ApiError> {
    let actor = caller.require_principal(Permission::SettingsManage)?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    if !media_type.eq_ignore_ascii_case("text/csv") {
        return Err(ApiError::UnsupportedMediaType);
    }
    let kind = ImportKind::parse(&kind).map_err(ApiError::BadRequest)?;
    let rows = import::parse(kind, &body)?;
    let count = match rows {
        Rows::Clients(rows) => state.store.import_clients(actor, &rows).await?,
        Rows::Matters(rows) => state.store.import_matters(actor, &rows).await?,
        Rows::Aliases(rows) => state.store.import_aliases(actor, &rows).await?,
        Rows::RateCards(rows) => state.store.import_rate_cards(actor, &rows).await?,
    };
    tracing::info!(kind = kind.as_str(), rows = count, "import applied");
    Ok(Json(ImportJson {
        kind: kind.as_str(),
        rows: count,
    }))
}
