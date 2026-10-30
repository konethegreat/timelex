// Written by Kone Tshivhinda & Claude
//! TimeLex gateway: walking skeleton.
//!
//! `GET /health` and `GET /v1/meta` are open. The gateway authenticates callers through the identity
//! service, routes three identity operations and MCP `tools/call` by header ([`mcp`]), enforces a
//! per-firm request budget and meters every handled request ([`proxy`]). Still to come: the outward
//! MCP aggregate. See `docs/ARCHITECTURE.md` and ADR-0001/0003/0012/0013.

pub mod config;
mod limits;
mod mcp;
mod meter;
mod proxy;

use axum::{http::StatusCode, response::Response, routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    name: &'static str,
    version: &'static str,
    /// Contract versions this build understands.
    contracts: Contracts,
    /// Honest status flag so clients never mistake the skeleton for a product.
    stage: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Contracts {
    activity_event: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

async fn meta() -> Json<Meta> {
    Json(Meta {
        name: "timelex-gateway",
        version: env!("CARGO_PKG_VERSION"),
        contracts: Contracts {
            activity_event: timelex_types::ACTIVITY_EVENT_SCHEMA_VERSION,
        },
        stage: "pre-alpha",
    })
}

/// Build the open routes only (`/health`, `/v1/meta`). Kept separate from `main` so tests can call it
/// without a socket or configuration.
pub fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/meta", get(meta))
        .fallback(not_found)
}

async fn not_found() -> Response {
    proxy::problem(StatusCode::NOT_FOUND, "not-found", "Not found", None)
}

/// The two listeners a running gateway needs: the public routes and the loopback internals.
///
/// They are separate so `/internal/usage` can be bound to a loopback address that tenants never reach.
pub struct Gateway {
    /// Routes intended for the network: open health and meta, plus authenticated service routes.
    pub public: Router,
    /// Operator-only endpoints; bind to loopback.
    pub internal: Router,
}

/// Build the full gateway: the open routes plus authenticated routing to the identity service.
/// Anything else, including the internal `authenticate` operation, is a 404.
pub fn app_with(config: config::Config) -> Result<Gateway, String> {
    let state = proxy::GatewayState::new(config)?;
    Ok(Gateway {
        public: app().merge(proxy::routes(state.clone())),
        internal: proxy::internal_routes(state),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get_json(path: &str) -> (u16, serde_json::Value) {
        let response = app()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn health_is_ok() {
        let (status, body) = get_json("/health").await;
        assert_eq!(status, 200);
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn meta_reports_version_contracts_and_stage() {
        let (status, body) = get_json("/v1/meta").await;
        assert_eq!(status, 200);
        assert_eq!(body["name"], "timelex-gateway");
        assert_eq!(body["contracts"]["activityEvent"], "1.0");
        assert_eq!(body["stage"], "pre-alpha");
    }

    #[tokio::test]
    async fn unknown_route_is_404() {
        let (status, _) = get_json("/nope").await;
        assert_eq!(status, 404);
    }
}
