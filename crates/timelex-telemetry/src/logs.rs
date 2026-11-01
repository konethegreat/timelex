// Written by Kone Tshivhinda & Claude
//! Structured logs with redaction applied before the record reaches a sink.

use std::fmt;

use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use crate::redact::{is_sensitive, scrub, REDACTED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogFormat {
    Plain,
    Json,
}

impl LogFormat {
    pub(crate) fn from_env() -> Result<Self, String> {
        match std::env::var("TIMELEX_LOG_FORMAT").as_deref() {
            Err(_) | Ok("plain") => Ok(Self::Plain),
            Ok("json") => Ok(Self::Json),
            Ok(other) => Err(format!(
                "TIMELEX_LOG_FORMAT={other:?} is not supported; use plain or json"
            )),
        }
    }
}

/// Everything an event recorded, with sensitive values already replaced.
#[derive(Default)]
struct Recorded {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl Recorded {
    fn push(&mut self, name: &str, raw: &str) {
        let value = if is_sensitive(name) {
            REDACTED.to_owned()
        } else {
            scrub(raw)
        };
        self.fields.push((name.to_owned(), value));
    }
}

impl Visit for Recorded {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(scrub(value));
        } else {
            self.push(field.name(), value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let raw = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(scrub(&raw));
        } else {
            self.push(field.name(), &raw);
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field.name(), &value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field.name(), &value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field.name(), &value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field.name(), &value.to_string());
    }
}

/// One event formatter for both formats, so a new field can never be redacted in only one of them.
pub(crate) struct RedactedFormat {
    service: &'static str,
    format: LogFormat,
}

impl RedactedFormat {
    pub(crate) fn new(service: &'static str, format: LogFormat) -> Self {
        Self { service, format }
    }
}

impl<S, N> FormatEvent<S, N> for RedactedFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut recorded = Recorded::default();
        event.record(&mut recorded);
        let metadata = event.metadata();
        let message = recorded.message.take().unwrap_or_default();
        let span = ctx.lookup_current().map(|span| span.name().to_owned());
        let time = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "-".to_owned());

        match self.format {
            LogFormat::Json => {
                let mut fields = Map::new();
                for (name, value) in recorded.fields {
                    fields.insert(name, Value::String(value));
                }
                let line = serde_json::json!({
                    "time": time,
                    "level": metadata.level().as_str(),
                    "service": self.service,
                    "target": metadata.target(),
                    "message": message,
                    "fields": fields,
                    "span": span,
                });
                writeln!(writer, "{line}")
            }
            LogFormat::Plain => {
                write!(
                    writer,
                    "{time} {:5} {} {}: {message}",
                    metadata.level(),
                    self.service,
                    metadata.target()
                )?;
                for (name, value) in &recorded.fields {
                    write!(writer, " {name}={value}")?;
                }
                if let Some(span) = &span {
                    write!(writer, " span={span}")?;
                }
                writeln!(writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Buffer {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = Buffer;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn capture(format: LogFormat, event: impl FnOnce()) -> String {
        let buffer = Buffer::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(buffer.clone())
                .event_format(RedactedFormat::new("test-service", format)),
        );
        tracing::subscriber::with_default(subscriber, event);
        buffer.text()
    }

    #[test]
    fn a_json_log_keeps_context_and_redacts_sensitive_fields() {
        let line = capture(LogFormat::Json, || {
            tracing::info!(
                token = "must-not-appear",
                route = "/v1/secrets",
                status = 201,
                "handled"
            );
        });
        assert!(!line.contains("must-not-appear"), "{line}");
        let value: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["level"], "INFO");
        assert_eq!(value["service"], "test-service");
        assert_eq!(value["message"], "handled");
        assert_eq!(value["fields"]["token"], "[redacted]");
        assert_eq!(value["fields"]["route"], "/v1/secrets");
        assert_eq!(value["fields"]["status"], "201");
    }

    #[test]
    fn a_plain_log_redacts_the_same_fields() {
        let line = capture(LogFormat::Plain, || {
            tracing::warn!(api_key = "must-not-appear", route = "/v1/secrets", "handled");
        });
        assert!(!line.contains("must-not-appear"), "{line}");
        assert!(line.contains("api_key=[redacted]"), "{line}");
        assert!(line.contains("route=/v1/secrets"), "{line}");
    }

    #[test]
    fn a_bearer_token_in_the_message_is_scrubbed_in_both_formats() {
        for format in [LogFormat::Plain, LogFormat::Json] {
            let line = capture(format, || {
                tracing::info!("presented Bearer not-a-real-token to identity");
            });
            assert!(!line.contains("not-a-real-token"), "{line}");
            assert!(line.contains("Bearer [redacted]"), "{line}");
        }
    }
}
