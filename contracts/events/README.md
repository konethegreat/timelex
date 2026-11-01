# Event contracts

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Events are immutable facts ([ADR-0006](../../docs/adr/0006-event-sourced-activity-and-ledger.md)). A stored event is read years after it was written, often by code that did not exist when it was produced, so its schema may only grow in ways every reader can survive.

Every event has a `schemaVersion` of the form `MAJOR.MINOR`, for example `1.0`. The major names the shape; the minor names the additions.

| Contract | Current version | Frozen released example |
|---|---|---|
| [`activity-event.schema.json`](activity-event.schema.json) | 1.0 | [`activity-event.v1.0.json`](../examples/activity-event.v1.0.json) |
| [`secret-event.schema.json`](secret-event.schema.json) | 1.0 | [`secret-event.v1.0.json`](../examples/secret-event.v1.0.json) |

## Rules within a major

These apply while the major is 1. A change that breaks any of them needs a new major.

1. **Only optional properties may be added.** Adding one bumps the minor. Producers write the newest minor they know; a reader of any `1.x` accepts every other `1.x`.
2. **Readers ignore properties they do not know**, at every level of the event, and do not treat a newer minor as an error. Unknown properties are dropped, never guessed at or copied forward. This is why the schemas do not set `additionalProperties: false`: strict field checking is a producer concern, not a reader's.
3. **Required properties are fixed.** Never add a required property, remove or rename one, change its type, or change what it means. Never narrow a constraint (`maxLength`, `maxItems`, `pattern`, `format`) and never turn an optional property into a required one.
4. **Enum vocabularies are fixed.** The values of `source.system`, `kind`, `participant.role` and similar fields name what an event *means*. A reader that meets a value it does not know cannot process the event honestly, so a new value is a new major, not a new minor. A new connector maps onto the existing vocabulary, or waits.
5. **Events are immutable.** A stored event is never edited to fit a newer schema. A correction is a new event that cites the earlier one ([ADR-0006](../../docs/adr/0006-event-sourced-activity-and-ledger.md)).
6. **Nothing reaches the stream invalid.** A producer validates against the current schema before publishing. There are no partly valid events, and a consumer does not repair one.
7. **A release freezes an example.** Each released minor keeps `contracts/examples/<name>.v<major>.<minor>.json`, which is never edited afterwards. `contracts/examples/<name>.json` is the current example used by docs and the SDK and is updated with each minor.
8. **The tests prove both directions.** `cargo test -p timelex-types` validates every frozen example against the current schema and the Rust types (a new reader still reads old events), and reads a current example carrying unknown fields (an old reader survives a newer minor). Changing a released example, or tightening the schema against it, fails CI.

## A breaking change

A breaking change is a new file with a new major in its name and its `schemaVersion`, for example `activity-event.v2.schema.json` with `schemaVersion: 2.x`. The v1 file stays, because the events already stored under it stay readable. A new major also needs a deprecation window: producers emit the old and the new shape until every consumer has moved, and consumers say which majors they accept.

## What is not an event version

`contracts/openapi/*.yaml` version their HTTP surface separately. A change to those contracts follows the rules in [`contracts/README.md`](../README.md).
