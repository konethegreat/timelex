// Written by Kone Tshivhinda & Claude
//! Process-wide telemetry: one subscriber, one tracer provider, one meter provider.
//!
//! Export is opt-in. Set `OTEL_EXPORTER_OTLP_ENDPOINT` (for example `http://127.0.0.1:4318`) to send
//! traces and metrics to a collector. Without it the process still creates trace ids and propagates
//! them inside the firm, which is what makes logs and hops correlate during local development.

use std::fmt;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider, Tracer};
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::logs::{LogFormat, RedactedFormat};

/// Why telemetry could not start. The caller turns this into the process's start-up error.
#[derive(Debug)]
pub enum InitError {
    /// The collector endpoint or its exporter could not be built.
    Exporter(String),
    /// `RUST_LOG` is not a valid filter directive.
    Filter(String),
    /// `TIMELEX_LOG_FORMAT` is neither `plain` nor `json`.
    LogFormat(String),
    /// A subscriber was already installed in this process.
    AlreadyInitialised(String),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exporter(message) => write!(f, "telemetry exporter: {message}"),
            Self::Filter(message) => write!(f, "RUST_LOG: {message}"),
            Self::LogFormat(message) => write!(f, "{message}"),
            Self::AlreadyInitialised(message) => {
                write!(f, "telemetry was already initialised: {message}")
            }
        }
    }
}

impl std::error::Error for InitError {}

/// Owns the providers so their batches are flushed when the process ends.
pub struct TelemetryGuard {
    tracer: Option<SdkTracerProvider>,
    meter: Option<SdkMeterProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.tracer.take() {
            if let Err(error) = provider.shutdown() {
                eprintln!("telemetry: tracer shutdown failed: {error}");
            }
        }
        if let Some(provider) = self.meter.take() {
            if let Err(error) = provider.shutdown() {
                eprintln!("telemetry: meter shutdown failed: {error}");
            }
        }
    }
}

/// Start telemetry for a service. Call once, before serving, and keep the guard alive.
pub fn init(service: &'static str) -> Result<TelemetryGuard, InitError> {
    let format = LogFormat::from_env().map_err(InitError::LogFormat)?;
    let endpoint = collector_endpoint();
    let tracer_provider = build_tracer_provider(service, endpoint.as_deref())?;
    let meter_provider = build_meter_provider(service, endpoint.as_deref())?;
    global::set_meter_provider(meter_provider.clone());
    install(service, format, tracer_provider.tracer(service), false)?;
    Ok(TelemetryGuard {
        tracer: Some(tracer_provider),
        meter: Some(meter_provider),
    })
}

/// Install a subscriber that exports spans to a provider owned by the caller. This is the test
/// seam: tests inspect spans without a collector, and a production build carries no testing code.
/// It may be called only once per process.
#[doc(hidden)]
pub fn install_for_tests(service: &'static str, tracer: Tracer) -> Result<TelemetryGuard, InitError> {
    install(service, LogFormat::Plain, tracer, true)?;
    Ok(TelemetryGuard {
        tracer: None,
        meter: None,
    })
}

fn install(service: &'static str, format: LogFormat, tracer: Tracer, quiet: bool) -> Result<(), InitError> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let filter = filter()?;
    let base = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_opentelemetry::layer().with_tracer(tracer));
    if quiet {
        let logs = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::io::sink)
            .event_format(RedactedFormat::new(service, format));
        base.with(logs)
            .try_init()
            .map_err(|error| InitError::AlreadyInitialised(error.to_string()))
    } else {
        let logs = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .event_format(RedactedFormat::new(service, format));
        base.with(logs)
            .try_init()
            .map_err(|error| InitError::AlreadyInitialised(error.to_string()))
    }
}

fn filter() -> Result<EnvFilter, InitError> {
    match std::env::var("RUST_LOG") {
        Ok(directives) => {
            EnvFilter::try_new(&directives).map_err(|error| InitError::Filter(error.to_string()))
        }
        Err(_) => Ok(EnvFilter::new("info")),
    }
}

fn collector_endpoint() -> Option<String> {
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn resource(service: &'static str) -> Resource {
    Resource::builder()
        .with_service_name(service)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build()
}

fn build_tracer_provider(
    service: &'static str,
    endpoint: Option<&str>,
) -> Result<SdkTracerProvider, InitError> {
    let mut builder = SdkTracerProvider::builder()
        .with_resource(resource(service))
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOn)));
    if let Some(endpoint) = endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint.to_owned())
            .build()
            .map_err(|error| InitError::Exporter(format!("cannot create the OTLP span exporter: {error}")))?;
        builder = builder.with_batch_exporter(exporter);
    }
    Ok(builder.build())
}

fn build_meter_provider(
    service: &'static str,
    endpoint: Option<&str>,
) -> Result<SdkMeterProvider, InitError> {
    let mut builder = SdkMeterProvider::builder().with_resource(resource(service));
    if let Some(endpoint) = endpoint {
        let exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_endpoint(endpoint.to_owned())
            .build()
            .map_err(|error| {
                InitError::Exporter(format!("cannot create the OTLP metric exporter: {error}"))
            })?;
        builder = builder.with_reader(PeriodicReader::builder(exporter).build());
    }
    Ok(builder.build())
}
