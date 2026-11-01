// Written by Kone Tshivhinda & Claude
//! TimeLex telemetry kit: OpenTelemetry traces and metrics, W3C context propagation, and structured
//! logs that redact secrets before they reach a sink. See ADR-0016.
//!
//! Services call [`init`] once at start-up, keep the returned guard alive for the process lifetime,
//! and wrap their router with [`instrument`]. Any outbound request made while a request span is
//! current should pass headers through [`inject`]; that is how a trace crosses the gateway into a
//! service. Export is opt-in through `OTEL_EXPORTER_OTLP_ENDPOINT`; without it the services still
//! create and propagate trace ids but send nothing.

mod http;
mod init;
mod logs;
mod redact;

pub use http::{inject, instrument};
pub use init::{init, install_for_tests, InitError, TelemetryGuard};
