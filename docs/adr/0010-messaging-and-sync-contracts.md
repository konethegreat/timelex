# ADR-0010: Messaging and synchronous contracts

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude

## Context
Services need to call each other (queries, commands) and react to each other's facts (events). We want few moving parts for a one-developer team, and every capability must also be an MCP tool.

## Decision
1. **Synchronous calls:** JSON over HTTP, described by OpenAPI 3.1 in `contracts/openapi/`. Internal calls use service identity (mTLS or signed service tokens) plus the tenant ID. gRPC is not adopted unless a measured need appears.
2. **Asynchronous events:** CloudEvents-style envelopes validated by JSON Schema in `contracts/events/`. Delivery via the **transactional outbox** into Postgres, relayed to a broker. Phase 1 uses a Postgres-backed queue (`LISTEN/NOTIFY` plus a polling relay); **NATS JetStream** is the planned broker when throughput or fan-out demands it. Consumers are idempotent.
3. **Idempotency keys** on every mutating REST route and MCP tool, because agents retry.
4. **Pagination, errors, tenancy and trace headers** are standardised in the platform kit: RFC 9457 problem details for errors, `traceparent` for tracing.
5. **MCP tool schemas are derived from the same Rust types** as the REST bodies; the catalogue in `contracts/mcp/` is checked against them.
6. **Versioning:** URL major version for REST; `schemaVersion` in event envelopes; additive changes only within a major version.

## Alternatives considered
- *Kafka:* heavy for the scale and team.
- *RabbitMQ / SQS:* fine, but NATS JetStream gives streams, replays and a single small binary for self-hosters.
- *gRPC internally:* faster and typed, but a second description language beside OpenAPI/JSON Schema and not what MCP/REST consumers need.

## Consequences
- Slightly higher latency than in-process calls; acceptable for our workloads.
- Contract tests (consumer-driven where useful) are part of CI from Phase 1.
- Self-hosters need only Postgres and (later) NATS.
