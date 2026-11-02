// Written by Kone Tshivhinda & Claude
//! TimeLex gateway: walking skeleton.
//!
//! `GET /health`, `GET /v1/meta`, the quick-capture page (`GET /capture`), the privacy page
//! (`GET /privacy`) and the data-flow register (`GET /v1/privacy/data-flow`) are open. The gateway
//! authenticates callers through the identity service, routes identity, vault, manual-capture and
//! privacy-exclusion operations and MCP `tools/call` by header ([`mcp`]), enforces a per-firm request
//! budget and meters every handled request ([`proxy`]). Still to come: the outward MCP aggregate. See
//! `docs/ARCHITECTURE.md` and ADR-0001/0003/0007/0012/0013/0023.

pub mod config;
mod limits;
mod mcp;
mod meter;
mod proxy;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{routing::get, Json, Router};
use serde::Serialize;

/// The quick-capture form, served from the gateway so it shares an origin with the API.
const QUICK_CAPTURE_PAGE: &str = include_str!("../assets/quick-capture.html");

/// The admin privacy page: the data-flow register and exclusion management.
const PRIVACY_PAGE: &str = include_str!("../assets/privacy.html");

/// The versioned technical data-flow register (ADR-0007). It is the source of truth for both the
/// page and the JSON export, so the two cannot drift.
const DATA_FLOW_REGISTER: &str = include_str!("../../../contracts/privacy/data-flow.v1.json");

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
        .route("/capture", get(quick_capture))
        .route("/privacy", get(privacy))
        .route("/v1/privacy/data-flow", get(data_flow))
        .fallback(not_found)
}

/// A self-contained page: no external asset can load, and nothing may frame it.
fn html_page(page: &'static str) -> Response {
    let mut response = Html(page).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; \
             connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}

async fn quick_capture() -> Response {
    html_page(QUICK_CAPTURE_PAGE)
}

async fn privacy() -> Response {
    html_page(PRIVACY_PAGE)
}

/// The register is public: it describes the platform, not a firm, and carries no tenant data.
async fn data_flow() -> Response {
    let mut response = DATA_FLOW_REGISTER.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    response
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
        public: timelex_telemetry::instrument(app().merge(proxy::routes(state.clone())), "gateway"),
        internal: timelex_telemetry::instrument(proxy::internal_routes(state), "gateway"),
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

    #[tokio::test]
    async fn quick_capture_page_is_self_contained_and_sends_to_the_capture_route() {
        let response = app()
            .oneshot(Request::builder().uri("/capture").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let headers = response.headers().clone();
        assert_eq!(
            headers[axum::http::header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(headers[axum::http::header::CACHE_CONTROL], "no-store");
        let policy = headers["content-security-policy"].to_str().unwrap();
        assert!(policy.contains("connect-src 'self'"), "{policy}");
        assert!(policy.contains("frame-ancestors 'none'"), "{policy}");

        let text =
            String::from_utf8(response.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(text.contains("Quick capture"));
        assert!(text.contains("/v1/capture/entries"));
        assert!(!text.contains("localStorage"), "the key is never persisted");
        assert!(!text.contains("sessionStorage"), "the key is never persisted");
        assert!(!text.contains('\u{2014}'), "UI copy uses no em dashes");
    }

    #[tokio::test]
    async fn privacy_page_is_self_contained_and_manages_exclusions() {
        let response = app()
            .oneshot(Request::builder().uri("/privacy").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let headers = response.headers().clone();
        assert_eq!(
            headers[axum::http::header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(headers[axum::http::header::CACHE_CONTROL], "no-store");
        let policy = headers["content-security-policy"].to_str().unwrap();
        assert!(policy.contains("default-src 'none'"), "{policy}");
        assert!(policy.contains("connect-src 'self'"), "{policy}");
        assert!(policy.contains("frame-ancestors 'none'"), "{policy}");

        let text =
            String::from_utf8(response.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(text.contains("Data-flow register"));
        assert!(text.contains("/v1/privacy/data-flow"));
        assert!(text.contains("/v1/privacy/exclusions"));
        assert!(text.contains("settings:manage"));
        assert!(!text.contains("localStorage"), "the key is never persisted");
        assert!(!text.contains("sessionStorage"), "the key is never persisted");
        assert!(!text.contains("innerHTML"), "render with textContent");
        assert!(!text.contains("http://"), "no external asset may load");
        assert!(!text.contains("https://"), "no external asset may load");
        assert!(!text.contains('\u{2014}'), "UI copy uses no em dashes");
    }

    #[tokio::test]
    async fn data_flow_register_is_versioned_json_without_tenant_data() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/privacy/data-flow")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let headers = response.headers().clone();
        assert_eq!(headers[axum::http::header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[axum::http::header::CACHE_CONTROL], "no-store");

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["version"], "1.0");
        assert_eq!(value["captureDefaults"]["contentCaptured"], false);
        assert_eq!(
            value["captureDefaults"]["domainExclusionsEnforcedBeforeStorage"],
            true
        );
        assert_eq!(
            value["exclusions"]["enforcedAtCapture"],
            serde_json::json!(["domain"])
        );
        assert_eq!(
            value["exclusions"]["appliesToExistingEvents"], false,
            "an exclusion must not claim to clean the past"
        );
        for key in ["dataCategories", "flows", "processors"] {
            assert!(
                value[key].as_array().is_some_and(|items| !items.is_empty()),
                "{key} must be a non-empty array"
            );
        }
        for flow in value["flows"].as_array().unwrap() {
            assert!(
                matches!(
                    flow["status"].as_str(),
                    Some("active" | "not_active" | "development_only")
                ),
                "unexpected flow status in {flow}"
            );
        }

        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!text.contains("tlx_"), "no credential may appear");
        assert!(!text.contains("sk-ant-"), "no credential may appear");
        assert!(!text.contains("BEGIN "), "no key material may appear");
    }
}
