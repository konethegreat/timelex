# capture-hub

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ framework (P2-02), manual quick capture (P2-08) and privacy exclusions (P2-07); real connectors arrive with P2-03/P2-05 and the redaction worker is still pending |
| **Language** | Rust |
| **Phase** | 2 |
| **Owns data** | `capture` schema: activity events, connector cursors, privacy exclusions |

## Purpose
Turn raw signals from Microsoft 365, Google and the desktop agent into normalised, idempotent activity events.

## What exists

- **The canonical event id.** [`crates/timelex-types`](../../crates/timelex-types/src/lib.rs) computes an activity event's id from the firm, `source.system`, `source.sourceId` and `startedAt`. The same signal always yields the same id, so a duplicate delivery is recognisable without any state ([ADR-0021](../../docs/adr/0021-idempotent-capture-and-replay.md), [the id rule](../../contracts/events/README.md)).
- **Idempotent ingest.** `POST /v1/events` recomputes the id and refuses a mismatch, then inserts with `ON CONFLICT (tenant_id, id) DO NOTHING`. The first delivery answers 201 and appends `activity.recorded` to the outbox in the same transaction; a repeat answers 200 with `created: false` and changes nothing. `activity_events` has no `UPDATE` or `DELETE` grant, so a stored event cannot be corrected, only superseded by a new one.
- **Connectors.** `Connector::fetch(cursor, limit)` returns one page of events and an opaque `next` cursor, or an error classified as retryable or permanent. `Runner::run_once` drains every connector of a firm from its stored cursor; a page and its new cursor commit in one transaction, so a failed page stores nothing and the old cursor stands.
- **Replay.** `Runner::replay` reads a connector from the beginning through the same idempotent path and does not touch the stored cursor. Re-running after a fix fills gaps without duplicating an event or disturbing the live schedule.
- **Manual quick capture.** `POST /v1/entries` turns a hand-recorded call, meeting or note into the same canonical activity event ([ADR-0022](../../docs/adr/0022-manual-quick-capture.md)). The client sends `{entryId, kind, startedAt, endedAt, description}`; the server derives the source (`manual`), the source id (the `entryId` UUID) and the event id, collapses the description into `subject.title` (at most 500 characters) and records metadata only. The `entryId` is the idempotency key, so a retried submit answers 200 with the same event. The gateway exposes it publicly at `POST /v1/capture/entries` and serves the mobile page at `/capture`.
- **Privacy exclusions.** A firm can exclude a `domain`, `client` or `matter`. Domain is enforced at capture: inside the tenant write transaction, before the insert, an event whose participant is at an excluded domain is dropped, so it is never stored and no `activity.recorded` is published ([ADR-0023](../../docs/adr/0023-privacy-exclusions.md)). Matching is case-insensitive, ignores a trailing dot and covers subdomains. A connector page still commits its cursor and reports the event as `excluded`, so one rule cannot stall capture. A single excluded ingest answers `200` with `created: false`, indistinguishable from a duplicate. Client and matter values are recorded now and enforced when attribution exists (Phase 3).
- **Metadata-only gate.** An event may carry a `contentRef` only when `contentCaptured` and `redactionApplied` are both true, and the reference is 1 to 512 characters. Every other combination is refused before storage. The P0-08 redaction module exists; its worker queue and container are pending.
- **Managing exclusions.** `GET`/`POST /v1/exclusions` and `DELETE /v1/exclusions/{kind}/{value}` need `settings:manage`. Adding is idempotent and keeps the first reason; removing is `404` when absent. The gateway exposes `GET`/`POST /v1/privacy/exclusions` and `DELETE /v1/privacy/exclusions/{kind}/{value}`.
- **Tenancy.** Every query runs in a `TenantTx` under `svc_capture`'s row-level security. The ingest and entry routes need a principal token with `capture:write`; the event's firm and actor must be the caller's. Exclusion management needs `settings:manage`.

## Contract
- **REST:** [`capture.v1.yaml`](../../contracts/openapi/capture.v1.yaml) (0.3.0): `POST /v1/events` (private), `POST /v1/entries` (private; the gateway's public path is `/v1/capture/entries`), `GET`/`POST /v1/exclusions` and `DELETE /v1/exclusions/{kind}/{value}` (private; public under `/v1/privacy/exclusions`), `/health`.
- **Events published:** `activity.recorded` (schema: [activity-event](../../contracts/events/activity-event.schema.json))
- **Events consumed:** `secret.rotated`, `secret.revoked` (not yet; P2-03)

## Not yet
- Real Microsoft Graph and Google connectors, subscriptions and renewal (P2-03, P2-04, P2-05)
- The redaction worker queue and container (the P0-08 module and tests exist); client and matter enforcement (Phase 3); backfill as MCP tasks (P2-11)
- No attribution decisions, no model calls except through `mcp-host`, no storage of unredacted content beyond the configured retention

## Run

```sh
# Migrate, then serve. The compose stack does both automatically.
export TIMELEX_MIGRATOR_DATABASE_URL=postgres://timelex_migrator@127.0.0.1:55433/timelex
cargo run -p timelex-capture-service -- migrate

export TIMELEX_CAPTURE_DATABASE_URL=postgres://svc_capture@127.0.0.1:55433/timelex
export TIMELEX_GATEWAY_PUBLIC_KEYS='k1=<base64 public key>'
cargo run -p timelex-capture-service
```

| Variable | Purpose |
|---|---|
| `TIMELEX_CAPTURE_DATABASE_URL` | `svc_capture` connection used to serve |
| `TIMELEX_MIGRATOR_DATABASE_URL` | `timelex_migrator` connection used by `migrate` |
| `TIMELEX_GATEWAY_PUBLIC_KEYS` | `kid=base64,...` keys that may sign internal tokens |
| `TIMELEX_CAPTURE_ADDR` | listen address, default `127.0.0.1:8084` |
| `TIMELEX_OUTBOX_RELAY` | `0` disables the in-process outbox relay |
| `TIMELEX_OUTBOX_INTERVAL_MS` | relay poll interval, default 500 |

The database tests need `TIMELEX_TEST_DATABASE_URL` (a superuser URL that can create a scratch database):

```sh
TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-capture-service
```

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0021](../../docs/adr/0021-idempotent-capture-and-replay.md), [ADR-0022](../../docs/adr/0022-manual-quick-capture.md), [ADR-0023](../../docs/adr/0023-privacy-exclusions.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
