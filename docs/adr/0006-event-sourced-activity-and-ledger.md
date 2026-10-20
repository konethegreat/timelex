# ADR-0006: Event-sourced activity and an append-only ledger

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Mirrors [ClearCut ADR-0004](https://github.com/konethegreat/clearcut/blob/main/docs/adr/0004-ingestion-and-narration-pipeline.md) ("events are the source of truth").

## Context
Every entry, narrative and bill line must be explainable and reproducible. LPC rules require records retained at least seven years and retrievable in readable form, with correction practices that preserve history ([research 03](../research/03-south-africa-compliance.md)).

## Decision
1. **Activity events** (`contracts/events/activity-event.schema.json`) are immutable facts about observed work: source, actor, time span, subject metadata, optional redacted content reference, and a stable idempotent ID.
2. **Time blocks, attributions, narratives and ledger entries are derived** and carry `evidence: [activity_event_id…]`, the model/prompt/policy version and a confidence.
3. **The time ledger is append-only.** A correction is a new entry that `supersedes` an earlier one; a void is an entry of kind `void`. Rows are never updated or deleted, enforced by database permissions (service role has INSERT and SELECT only) and checked in tests.
4. **Money is integer minor units (cents)**, durations are integer seconds; rounding to billing units is done by `timelex-units` and the rule used is stored on the entry.
5. **Hash-chained audit records** per tenant (each record includes the hash of the previous) for tamper evidence; periodic anchors can be exported for the firm's auditor.
6. **Outbox pattern.** A state change and its outbound event are written in one database transaction; a relay publishes to subscribers ([ADR-0010](0010-messaging-and-sync-contracts.md)).
7. **Retention** is a tenant setting with a seven-year floor for financial records; raw captured content has a much shorter, configurable life than metadata.

## Alternatives considered
- *CRUD tables with an audit trigger.* Simpler; weaker guarantees and harder explanations.
- *Full event sourcing with replay as the only read model.* Elegant, costly for one developer. We use event-**sourced facts at the edges** (activity, decisions) and ordinary tables for read models.

## Consequences
- Reads that show "current state" use views over superseding chains (as ClearCut's event views do).
- Storage grows; archival tiering is needed.
- GDPR-style erasure does not map directly; POPIA retention and deletion rules need a documented approach (redact content, keep minimal ledger facts).
