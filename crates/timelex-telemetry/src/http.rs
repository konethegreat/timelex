// Written by Kone Tshivhinda & Claude
//! Request spans, metrics and W3C trace-context propagation for axum services.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::trace::TraceContextExt;
use opentelemetry::KeyValue;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Wrap a router so every request gets a span, latency and status metrics, and any incoming W3C
/// trace continues into this service. Call it after routes are added: the layer reads the matched
/// route, which only exists once routing has run.
pub fn instrument(router: Router, service: &'static str) -> Router {
    let metrics = Arc::new(Metrics::new(&global::meter("timelex")));
    router.layer(axum::middleware::from_fn(move |request: Request, next: Next| {
        let metrics = Arc::clone(&metrics);
        async move { trace(service, metrics, request, next).await }
    }))
}

/// Inject the current trace context into outbound headers. A no-op when no span is active, so a
/// caller can always call it.
pub fn inject(headers: &mut HeaderMap) {
    let context = tracing::Span::current().context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut HeaderInjector(headers));
    });
}

async fn trace(_service: &'static str, metrics: Arc<Metrics>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned());
    let name = format!("{} {}", method, route.as_deref().unwrap_or(&path));
    let parent =
        global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(request.headers())));

    let span = tracing::info_span!(
        "http.server",
        "otel.name" = %name,
        "otel.kind" = "server",
        "http.request.method" = %method,
        "url.path" = %path,
        "http.route" = tracing::field::Empty,
        "http.response.status_code" = tracing::field::Empty,
    );
    if let Some(route) = &route {
        span.record("http.route", route.as_str());
    }
    if parent.span().span_context().is_valid() {
        // A valid parent cannot be refused; if it somehow is, this span starts a new trace.
        let _ = span.set_parent(parent);
    }

    let started = Instant::now();
    // The span ends when the instrumented future finishes, so the status must be recorded inside it.
    let response = {
        let inner_span = span.clone();
        async move {
            let response = next.run(request).await;
            let status = response.status();
            inner_span.record("http.response.status_code", i64::from(status.as_u16()));
            if status.is_server_error() {
                inner_span.record("otel.status_code", "ERROR");
            }
            response
        }
        .instrument(span)
        .await
    };
    let status = response.status();
    metrics.record(
        &method,
        route.as_deref().unwrap_or("unmatched"),
        status,
        started.elapsed(),
    );
    response
}

struct Metrics {
    requests: Counter<u64>,
    duration: Histogram<f64>,
}

impl Metrics {
    fn new(meter: &Meter) -> Self {
        Self {
            requests: meter
                .u64_counter("timelex.http.server.requests")
                .with_description("HTTP requests handled, by method, route and status")
                .build(),
            duration: meter
                .f64_histogram("timelex.http.server.duration")
                .with_unit("s")
                .with_description("HTTP request duration in seconds, by method, route and status")
                .build(),
        }
    }

    fn record(&self, method: &Method, route: &str, status: StatusCode, elapsed: std::time::Duration) {
        let attributes = [
            KeyValue::new("http.request.method", method.as_str().to_owned()),
            KeyValue::new("http.route", route.to_owned()),
            KeyValue::new("http.response.status_code", i64::from(status.as_u16())),
        ];
        self.requests.add(1, &attributes);
        self.duration.record(elapsed.as_secs_f64(), &attributes);
    }
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(HeaderName::as_str).collect()
    }
}

struct HeaderInjector<'a>(&'a mut HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let name = HeaderName::from_bytes(key.as_bytes());
        let value = HeaderValue::from_str(&value);
        if let (Ok(name), Ok(value)) = (name, value) {
            self.0.insert(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry::trace::{SpanId, TracerProvider};
    use opentelemetry::Value;
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
    use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
    use tower::ServiceExt;

    use super::*;

    const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
    const CLIENT_SPAN_ID: &str = "00f067aa0ba902b7";

    fn test_tracer() -> (SdkTracerProvider, InMemorySpanExporter) {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        (provider, exporter)
    }

    async fn handler() -> String {
        let mut headers = HeaderMap::new();
        inject(&mut headers);
        headers
            .get("traceparent")
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default()
    }

    /// The subscriber is global, so both scenarios live in one test. That is also the honest order:
    /// a root request first, then one that continues a caller's trace.
    #[tokio::test]
    async fn trace_context_is_extracted_injected_and_exported() {
        let (provider, exporter) = test_tracer();
        let _guard = crate::init::install_for_tests("test", provider.tracer("test"))
            .expect("the test subscriber should install");

        let app = instrument(
            Router::new()
                .route("/hop", get(handler))
                .route("/root", get(handler)),
            "test",
        );

        // 1. No incoming trace: the service starts one and still injects it onwards.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/root")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let outgoing = String::from_utf8(body.to_vec()).unwrap();
        let parts: Vec<&str> = outgoing.split('-').collect();
        assert_eq!(parts.len(), 4, "{outgoing}");
        assert_ne!(parts[1], TRACE_ID);

        // 2. An incoming traceparent: the trace id survives and the parent is the caller's span.
        let incoming = format!("00-{TRACE_ID}-{CLIENT_SPAN_ID}-01");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/hop")
                    .header("traceparent", &incoming)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let outgoing = String::from_utf8(body.to_vec()).unwrap();
        let parts: Vec<&str> = outgoing.split('-').collect();
        assert_eq!(parts.len(), 4, "{outgoing}");
        assert_eq!(parts[0], "00");
        assert_eq!(parts[1], TRACE_ID, "the trace id must be preserved");
        assert_ne!(parts[2], CLIENT_SPAN_ID, "the service starts its own span");

        let spans = exporter.get_finished_spans().unwrap();
        let server = spans
            .iter()
            .find(|span| span.span_context.trace_id().to_string() == TRACE_ID)
            .expect("the request span should be exported");
        assert_eq!(server.name, "GET /hop");
        assert_eq!(server.parent_span_id, SpanId::from_hex(CLIENT_SPAN_ID).unwrap());
        let route = server
            .attributes
            .iter()
            .find(|kv| kv.key.as_str() == "http.route")
            .and_then(|kv| match &kv.value {
                Value::String(value) => Some(value.to_string()),
                _ => None,
            });
        assert_eq!(route.as_deref(), Some("/hop"));
        let status = server
            .attributes
            .iter()
            .find(|kv| kv.key.as_str() == "http.response.status_code")
            .and_then(|kv| match kv.value {
                Value::I64(value) => Some(value),
                _ => None,
            });
        assert_eq!(status, Some(200));

        let root = spans
            .iter()
            .find(|span| span.name == "GET /root")
            .expect("the root request span should be exported");
        assert_eq!(root.parent_span_id, SpanId::INVALID);
    }

    #[test]
    fn one_request_records_a_counter_and_a_duration_histogram() {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_reader(PeriodicReader::builder(exporter.clone()).build())
            .build();
        let metrics = Metrics::new(&provider.meter("test"));
        metrics.record(
            &Method::GET,
            "/v1/secrets",
            StatusCode::OK,
            std::time::Duration::from_millis(3),
        );
        provider.force_flush().unwrap();

        let finished = exporter.get_finished_metrics().unwrap();
        let names: Vec<&str> = finished
            .iter()
            .flat_map(|resource| resource.scope_metrics())
            .flat_map(|scope| scope.metrics())
            .map(|metric| metric.name())
            .collect();
        assert!(names.contains(&"timelex.http.server.requests"), "{names:?}");
        assert!(names.contains(&"timelex.http.server.duration"), "{names:?}");
    }
}
