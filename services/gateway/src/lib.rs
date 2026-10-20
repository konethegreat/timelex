// Written by Kone Tshivhinda & Claude
//! TimeLex gateway: walking skeleton.
//!
//! Today: `GET /health` and `GET /v1/meta`. Later (Phase 1): authN, tenant resolution, rate
//! limits, metering, header-based routing and outward MCP aggregation. See
//! `docs/ARCHITECTURE.md` and ADR-0001/0003.

use axum::{routing::get, Json, Router};
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

/// Build the router. Kept separate from `main` so tests can call it without a socket.
pub fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/meta", get(meta))
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
