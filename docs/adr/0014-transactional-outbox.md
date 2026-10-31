# ADR-0014: Transactional outbox on Postgres

Status: Accepted · Date: 2026-10-31 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0006](0006-event-sourced-activity-and-ledger.md), [ADR-0010](0010-messaging-and-sync-contracts.md) and [ADR-0011](0011-tenancy-enforcement-and-database-roles.md); implements P1-06.

## Context

A state change must never be visible without its event, and an event must never describe a change that rolled back. Services that own tenant data run under row-level security that no runtime role can bypass ([ADR-0011](0011-tenancy-enforcement-and-database-roles.md)). Phase 1 has Postgres but no broker, and the gateway is deliberately database-free.

The first events are `secret.created`, `secret.updated`, `secret.tested`, `secret.rotated` and `secret.revoked` from the vault. Later phases add activity, ledger and billing events.

## Decision

1. **One `platform.outbox` table serves every service.** It holds `seq` (identity, the queue order), `id` (the envelope id, unique), `tenant_id`, `event_type`, `payload` (jsonb, the whole envelope), `created_at`, `published_at`, `attempts`, `next_attempt_at` and `last_error`. A partial index covers undelivered rows in claim order.

2. **The writer inserts the event on its own connection, inside its own transaction.** `timelex_outbox::append` takes the writer's `PgConnection`, so the state change, the audit entry and the event commit or roll back together. No separate publisher process can observe a change that did not commit.

3. **The relay claims a batch, publishes it, then marks it.** Claiming is `UPDATE … WHERE id IN (SELECT … ORDER BY seq LIMIT $1 FOR UPDATE SKIP LOCKED)`, which lets several relay processes run without handing the same row to two of them. A claim leases the row for 60 seconds by moving `next_attempt_at`. Publishing happens before `published_at` is set, so a crash between the two causes a redelivery, never a lost event. A failed publish records `last_error` and retries with exponential backoff (2^attempts seconds, capped at 300).

4. **Delivery is at least once; the effect is exactly once because consumers dedupe on the envelope `id`.** Ids are `ev_` plus 32 lowercase hex characters from a UUIDv4, unique in the table. A test delivers the same event twice to an idempotent consumer and asserts one effect.

5. **Envelopes are contracts first.** [`contracts/events/secret-event.schema.json`](../../contracts/events/secret-event.schema.json) is JSON Schema 2020-12 and is mirrored by `timelex-types`, with golden-example tests in both directions. The secret events carry metadata only (kind, label, last four, test result) and never a value; the schema requires `result` only for `secret.tested` and forbids it otherwise.

6. **The relay runs inside the service process in Phase 1.** The vault starts it by default; `TIMELEX_OUTBOX_RELAY=0` disables it and `TIMELEX_OUTBOX_INTERVAL_MS` sets the poll interval (default 500 ms). The interim sink writes one JSON line per event to stdout behind a `Publisher` trait. The broker (NATS JetStream) replaces the sink, not the table.

## Alternatives considered

- **One outbox table per service schema.** The relay must read across every tenant, but each service role is forced under row-level security and has no bypass. A relay role would need either `BYPASSRLS` (which defeats ADR-0011) or a policy exception per schema. Rejected.
- **A row per consumer or per queue.** There are no independent consumers yet, and fan-out is the broker's job. A second queue would duplicate the same event and split its ordering. Rejected.
- **`LISTEN/NOTIFY` as the delivery path.** A notification is not durable: a relay that is down when the notification fires misses it. Polling is the durable path; `NOTIFY` can later shorten latency without changing delivery semantics.
- **Publishing from a database trigger.** SQL cannot reach the broker, and a trigger failure would change application behaviour. Rejected.
- **A dedupe table in the relay.** The relay's obligation is delivery; the consumer's obligation is effect. P2-02 adds the consumer-side store.

## Consequences

- **The relaying role can read every tenant's event metadata.** Today that is the service role itself, which needs `SELECT` to claim rows. Payloads never contain secret values, and column-limited grants stop a producer rewriting a payload or clearing `published_at`. Splitting a dedicated relay role from producer roles is the next tightening.
- **One table is a contention point.** `SKIP LOCKED` keeps workers from blocking each other; if volume demands it, partition by month or service without changing the envelope.
- **`seq` is insert order, not commit order.** A consumer must not treat it as a global total order across services; events carry their own facts and timestamps.
- **No dead-letter queue yet.** A permanently failing event retries forever with capped backoff while `attempts` and `last_error` grow; alerting on attempts is required before the first external consumer ships.
- **Published rows are kept.** A retention or archival job is needed before the table's growth matters.
- **The gateway stays database-free.** It will produce events through the service it calls, never through its own connection.
