# ADR-0016: One telemetry kit for every service

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0001](0001-service-boundaries-and-contract-first.md), [ADR-0004](0004-byo-keys-and-secrets-vault.md), [ADR-0012](0012-internal-service-authentication.md) and [ADR-0013](0013-gateway-limits-and-metering.md); implements P1-10 and the server half of the observability section in [ARCHITECTURE](../ARCHITECTURE.md).

## Context

The gateway, identity and vault already write operational records (the gateway's stdout meter, the vault's JSON-line outbox relay), but a request that crosses the gateway into a service has no shared identifier. When a firm reports "the secret save failed", an operator cannot tell which gateway request reached which service, how long each hop took, or whether the service even saw the call. P1-11 will generate new services, so whatever observability they get must be a workspace dependency, not a copy-paste ritual.

Constraints: the workspace forbids `unsafe`, runs `clippy -D warnings`, and keeps domain crates free of I/O. Secrets must never reach a log, an event or a span ([ADR-0004](0004-byo-keys-and-secrets-vault.md)). A developer must be able to run the stack with no collector and still correlate logs; a production deployment must be able to add one without a code change.

## Decision

1. **`crates/timelex-telemetry` is the workspace's only OpenTelemetry dependency.** It re-exports the three entry points a service needs: `init(service)`, `instrument(router, service)` and `inject(&mut HeaderMap)`. Services do not depend on `opentelemetry`, `opentelemetry_sdk` or `opentelemetry-otlp` directly. Versions are pinned in one manifest (`opentelemetry` 0.33, `tracing-opentelemetry` 0.34, `tracing-subscriber` 0.3).

2. **Export is opt-in, propagation is always on.** `init` builds a tracer and meter provider with or without a collector. When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, a batch span exporter and a periodic metric reader send OTLP/HTTP protobuf to it. When it is unset, spans still get real trace and span ids, are still propagated, and are simply not exported. Local development therefore correlates hops without running a collector, and a deployment switches export on with one environment variable.

3. **W3C `traceparent` is the only propagation format.** `TraceContextPropagator` is installed once per process. The gateway's middleware extracts an incoming trace and starts its server span as that trace's child; every internal call (`authenticate`, `forward`, `relay`) injects the current context, so identity and the vault join the same trace. Services extract from their own inbound request in the same middleware. No proprietary headers, no B3.

4. **One server span per HTTP request, named and attributed by the router.** The middleware runs after routing, so `otel.name` is `METHOD /matched/route` (the raw path when nothing matched) and the span carries `http.request.method`, `url.path`, `http.route`, `http.response.status_code` and `otel.kind = server`. A 5xx also records `otel.status_code = ERROR`. Attributes are constants: route patterns, methods and status codes, never tenant ids, key ids or request bodies.

5. **Two HTTP metrics.** `timelex.http.server.requests` (counter) and `timelex.http.server.duration` (histogram, seconds), attributed by method, route and status. An unmatched request is labelled `unmatched` rather than by its path, so cardinality stays bounded by the route table.

6. **Logs are redacted structured records, not a second telemetry pipeline.** One subscriber hosts the OpenTelemetry layer and a `fmt` layer whose event format is a TimeLex type: JSON or plain text (`TIMELEX_LOG_FORMAT`, default plain), prefixed with the service name. Fields whose names match a sensitive vocabulary (`authorization`, `cookie`, `token`, `secret`, `password`, `value`, `api_key`, `master_key`, `private_key`, and the rest in `redact.rs`) are replaced with `[redacted]`, and bearer tokens inside free text are scrubbed. `RUST_LOG` filters records as usual.

7. **Sampling is parent-based always-on.** A root request is sampled; a caller's decision is honoured. There is no tail sampler and no per-tenant rule in Phase 1.

8. **Tests inspect real spans.** `install_for_tests(service, tracer)` installs the same subscriber with a caller-owned tracer and a sink writer, so a test can drive requests and read exported spans from an in-memory exporter. It is `#[doc(hidden)]`, and the production binary never links it into a code path. The gateway test asserts one caller trace containing the gateway span and the service's child span, with the service receiving the gateway's `traceparent`.

## Alternatives considered

- **Each service wiring `opentelemetry` itself.** Three copies of the same middleware is how the redaction list, the propagation format and the span names drift apart; P1-11 would then generate a fourth. Rejected.
- **SDK defaults with deployment policy on top.** Origin-style defaults are how an unconfigured service starts serving unvalidated traces; more practically, SDK defaults change between minor versions without a compile error here. The kit states its settings in code and tests them.
- **Export enabled by default with a localhost collector.** A process that cannot reach its collector fills its batch queue and spends time retrying, and the dev stack would need a collector container just to stay quiet. Opt-in keeps the compose stack honest about what it runs.
- **B3 or a proprietary correlation header.** `traceparent` is the W3C standard, is what every collector and SDK understands, and is the header a future third-party agent can produce. Rejected.
- **Prometheus text endpoint instead of OTLP metrics.** It adds an HTTP surface on every service and a scrape configuration, while OTLP reuses the same exporter and endpoint as traces. Rejected for now; a collector can expose Prometheus downstream.
- **A tail sampler in the kit.** It needs a collector or a local buffer with a decision window; the Phase 1 volume does not justify it. Parent-based sampling can be tightened later without changing call sites.
- **Correlating logs by injecting the trace id into every record.** It is useful, but the `fmt` layer's span context already carries it when a record is emitted inside a span, and adding it to records emitted outside one would require wrapping the whole process. Left for the collector.

## Consequences

- **No collector in the dev stack yet.** `deploy/docker-compose.yml` sets `TIMELEX_LOG_FORMAT=json`, `RUST_LOG=info` and documents `OTEL_EXPORTER_OTLP_ENDPOINT`; pointing it at a host collector (for example `http://host.docker.internal:4318`) is the whole setup. Adding an OpenTelemetry Collector service is a later deployment change, not a code change.
- **Logs stay on stdout.** They are not shipped over OTLP, so a deployment must collect stdout as it already does. Redaction happens before the record is formatted, so a collector never sees the sensitive value.
- **Name-based redaction is a safety net, not a licence to log secrets.** A field called `body` containing a key would pass; the rule remains that values are never logged, and the vault's tests scan its output for them.
- **Batches are lost on a hard kill.** `TelemetryGuard` shuts the providers down when `serve` returns, which flushes on Ctrl-C and on a graceful stop; `SIGKILL` loses whatever was queued. Export intervals accept that.
- **Route labels are the route table.** A service that adds a route adds a metric series; that is bounded and reviewed, unlike path labels.
- **The kit grows with what services need.** MCP request spans, database query spans and per-model-call records are added here (or in a sibling crate) rather than in each service, and each addition comes with a contract test.
