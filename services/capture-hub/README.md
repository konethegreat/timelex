# capture-hub

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ framework (P2-02); connectors arrive with P2-03/P2-05 |
| **Language** | Rust |
| **Phase** | 2 |
| **Owns data** | `capture` schema: activity events, connector cursors |

## Purpose
Turn raw signals from Microsoft 365, Google and the desktop agent into normalised, idempotent activity events.

## What exists

- **The canonical event id.** [`crates/timelex-types`](../../crates/timelex-types/src/lib.rs) computes an activity event's id from the firm, `source.system`, `source.sourceId` and `startedAt`. The same signal always yields the same id, so a duplicate delivery is recognisable without any state ([ADR-0021](../../docs/adr/0021-idempotent-capture-and-replay.md), [the id rule](../../contracts/events/README.md)).
- **Idempotent ingest.** `POST /v1/events` recomputes the id and refuses a mismatch, then inserts with `ON CONFLICT (tenant_id, id) DO NOTHING`. The first delivery answers 201 and appends `activity.recorded` to the outbox in the same transaction; a repeat answers 200 with `created: false` and changes nothing. `activity_events` has no `UPDATE` or `DELETE` grant, so a stored event cannot be corrected, only superseded by a new one.
- **Connectors.** `Connector::fetch(cursor, limit)` returns one page of events and an opaque `next` cursor, or an error classified as retryable or permanent. `Runner::run_once` drains every connector of a firm from its stored cursor; a page and its new cursor commit in one transaction, so a failed page stores nothing and the old cursor stands.
- **Replay.** `Runner::replay` reads a connector from the beginning through the same idempotent path and does not touch the stored cursor. Re-running after a fix fills gaps without duplicating an event or disturbing the live schedule.
- **Tenancy.** Every query runs in a `TenantTx` under `svc_capture`'s row-level security. The ingest route needs a principal token with `capture:write`; the event's firm and actor must be the caller's.

## Contract
- **REST:** [`capture.v1.yaml`](../../contracts/openapi/capture.v1.yaml): `POST /v1/events`, `/health`. Private interface only; the gateway does not route it until the first public caller (P2-08).
- **Events published:** `activity.recorded` (schema: [activity-event](../../contracts/events/activity-event.schema.json))
- **Events consumed:** `secret.rotated`, `secret.revoked` (not yet; P2-03)

## Not yet
- Real Microsoft Graph and Google connectors, subscriptions and renewal (P2-03, P2-04, P2-05)
- Privacy filter, exclusions and redaction (P2-07); manual quick capture (P2-08); backfill as MCP tasks (P2-11)
- Gateway routing (P2-08)
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

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0021](../../docs/adr/0021-idempotent-capture-and-replay.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
