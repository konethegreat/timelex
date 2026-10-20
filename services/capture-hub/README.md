# capture-hub

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified |
| **Language** | Rust |
| **Phase** | 2 |
| **Owns data** | `capture` schema: connectors, subscriptions, cursors, activity events, redacted content references |

## Purpose
Turn raw signals from Microsoft 365, Google and the desktop agent into normalised, idempotent activity events.

## Responsibilities
- Connector trait; Microsoft Graph mail/calendar/Teams, later Google
- Webhook receipt with `clientState` validation; fetch-then-trust
- Subscription renewal and lifecycle-notification handling (Outlook subscriptions expire in under 7 days)
- Backfill as long-running MCP Tasks; replay from cursors
- Privacy filter pipeline: exclusions, redaction (calls `py-worker`), metadata-only default

## Contract
- **REST:** `/v1/connectors`, `/v1/connectors/{id}/subscriptions`, `/v1/events` (ingest from desktop agent and quick-capture), `/v1/backfills`
- **MCP tools:** `list_connectors` (R), `start_backfill` (W), `quick_capture` (W)
- **Events published:** `activity.recorded` (schema: [activity-event](../../contracts/events/activity-event.schema.json))
- **Events consumed:** `secret.rotated`, `secret.revoked`

## Non-goals
- No attribution decisions
- No model calls except through `mcp-host`
- No storage of unredacted content beyond the configured retention

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
