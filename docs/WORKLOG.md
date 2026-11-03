# Worklog

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Append an entry at the end of every working session (newest first).

## 2026-11-03 (latest): P3-04 — matters keep the firm's own keys

**Trigger:** Attribution cannot name anything until the platform knows the firm's clients and
matters. P3-05 measures precision against the gold set, whose labels use the firm's own `cl_…` and
`mt_…` identifiers, so the records had to accept those identifiers unchanged. P1-02's acceptance
criterion for this task is import from CSV.

**Done**
- [`services/matters`](../services/matters) now owns `clients`, `matters`, `aliases` and
  `rate_cards` in the `matters` schema, all under the standard tenant RLS policy. `clientId` and
  `matterId` are the firm's own keys; there is no surrogate public id. A matter's reference has a
  trimmed, lower-cased `reference_key`, unique per firm.
- Aliases are explicit hard keys (`matter_reference`, `client_name`, `client_domain`), canonicalised
  on the way in, unique per kind and value, with a composite foreign key keeping a matter alias in
  the same client. The same value for the same target is idempotent; another target is a 409.
  Creating a matter invents no aliases, so a rename cannot leave a stale key.
- One rate card per matter (`PUT` replaces it): hourly cents, unit minutes, rounding, minimum units,
  with the contract defaults applied when omitted.
- CSV import for the four kinds: strict headers, at most 5 000 rows and 2 MiB, `text/csv` only, one
  transaction, upserts by natural key, logical row numbers in errors and no cell value echoed.
- HTTP layer with RFC 9457 problems. Reads need `time:read`; writes and imports need
  `settings:manage`; the firm comes from the verified internal token.
- [ADR-0027](adr/0027-matters-natural-keys-and-import.md), contract 0.1.0
  ([`matters.v1.yaml`](../contracts/openapi/matters.v1.yaml)), service README, ARCHITECTURE, TASKS and
  INDEX are updated. The image and dev compose now build and run the service; the dev login file
  grants `svc_matters` LOGIN.

**Verification**
- 43 tests pass against Postgres 17: 14 domain unit tests, 13 parser tests, 11 HTTP tests (health,
  authentication, per-route permissions, tenancy, conflicts, validation, media type, 404) and 5
  database import tests.
- The import tests prove a bad row rolls the whole file back, re-importing updates rather than
  duplicates, a reference cannot arrive as two matters, another firm cannot see or attach to these
  rows, and the gold-set firm imports completely: 8 clients, 24 matters (2 closed), 32 aliases and 24
  rate cards, with every matter named by a label present.
- `cargo fmt --all`, `cargo clippy -p timelex-matters-service --all-targets -- -D warnings` and the
  full service test suite are clean. CI runs the same suite against its own Postgres service.

**Decisions**
- Natural keys rather than surrogate ids with an `externalId` map: the gold set and a practice-system
  export both round-trip, and P3-05 needs no translation layer.
- Explicit aliases rather than aliases invented on write: a stale hard key silently attributes work
  to the wrong matter, which is worse than an absent one.
- All-or-nothing import rather than skipping bad rows: a partial load leaves the firm unsure what
  exists, and the row-numbered refusal is what makes the whole-file retry safe.
- No gateway route and no `matter.opened`/`matter.closed` events yet. Routing is per path in the
  gateway and belongs with the deploy that fronts the service; the events need a frozen contract and
  a consumer.

**Next:** P3-05 attribution stages 0–1 (hard keys, history) over the gold set and the segmentation
blocks. Contacts, the conflict index, MCP tools and a public gateway route remain later increments.

## 2026-11-02: P3-03 — segmentation keeps the time arithmetic deterministic

**Trigger:** The gold set now exists, but attribution's first stage did not: a day of mail, meetings
and document work is still a list of overlapping signals, not a set of intervals that can be reviewed
or billed. Segmentation decides how much time the rest of the pipeline is arguing about, so it cannot
be a prompt and it cannot vary between runs.

**Done**
- [`crates/timelex-segmentation`](../crates/timelex-segmentation) is a pure library over
  `ActivityEvent`: no I/O, no clock, no model, no floats. It refuses mixed tenants and mixed actors,
  treats intervals as whole-second half-open ranges (start down, end up), and drops point events.
- Grouping has one priority: `threadRef`, then `documentRef`, then the sorted lower-cased participant
  domains, then the event's own id. A group joins across gaps of at most five minutes (inclusive).
  Meetings, calls and manual entries are anchors and are placed before mail, document and desktop
  work; every later group is clipped against the claimed union.
- The invariants are explicit: blocks never share a second, block durations sum to the union of the
  input intervals, and each block cites the events that intersect it. An anchor fully covering a
  passive group leaves no passive block; a call inside a document block splits it in two.
- Determinism is structural: `BTreeMap` groups, `(class, first start, group key)` order, and a total
  order on blocks. Reversing and shuffling the input yields the identical vector.
- [ADR-0026](adr/0026-deterministic-segmentation.md) records the rules, the clipping choice and its
  consequences. TASKS, ARCHITECTURE and INDEX are updated. The gold set has its first Rust consumer:
  two tests read `eval/synthetic/corpus/events.json` and segment all 220 events per actor.

**Verification**
- 30 tests pass: 4 interval unit tests and 26 integration tests, including gap boundaries, same-class
  and anchor/passive overlap, full cover, mixed tenant/actor refusal, end-before-start, sub-second
  outward rounding, a 200-event generated set whose every claimed second is inserted into a set (a
  duplicate fails), union equality, and input-order independence over 50 shuffles.
- The corpus test asserts a stronger per-event property: every input interval is covered by the
  output blocks. It caught a real case the first version of the assertion missed: a document view at
  10:44 on 22 September sits wholly inside a different document's edit block, so it is covered rather
  than cited as evidence. That is the intended equal-class rule, now stated in the ADR.
- `cargo fmt --all -- --check`, `cargo clippy -p timelex-segmentation --all-targets -- -D warnings`
  and `cargo test -p timelex-segmentation` are clean; the docs link check passes.

**Decisions**
- Clipping rather than allowing overlap: an overlapping block would let two entries bill one set of
  minutes. The losing events stay in the ledger, so a later overlap report can show them.
- Class-level precedence rather than per-kind precedence: passive work on the same thread should not
  split merely because one event is a view and another an edit.
- No rounding in segmentation: six-minute units belong to `timelex-units` and a firm's fee agreement.

**Next:** P3-04 minimal matters (clients, matters, aliases, rate cards, CSV import) is the next
unblocked task, then P3-05 can run the hard-key and history stages over this corpus and these blocks.

## 2026-11-02: P3-01 — a measured pipeline needs a corpus before it needs a model

**Trigger:** Gate A is a measured claim, and the consented real corpus cannot be the first source: it
waits on the P3-02 POPIA protocol, is private and cannot enter CI. The synthetic gold set is the long
pole for the gate, and starting it late would leave attribution and segmentation with nothing honest
to regress against. The redaction worker was the other candidate, but no connector produces content
yet, so a queue and container would be built against an imagined producer.

**Done**
- [`eval/synthetic`](../eval/synthetic) is a Python package that builds one fictional firm from a
  seed: 6 users, 8 clients, 24 matters (2 closed), 50 mail threads, 38 meetings and 220 activity
  events across 20 business days, plus internal mail, newsletters, excluded-domain mail and meetings,
  desktop document work and manual calls. Every address uses `.example` or `.test`.
- Output is canonical JSON (sorted keys, two-space indent, LF) with a manifest of the seed, generator
  version, counts and per-file SHA-256 and byte length. `generate` writes `firm.json`, `threads.json`,
  `calendar.json`, `events.json`, `labels.json` and `manifest.json`; `check` regenerates in memory
  and reports a missing, unexpected or differing file.
- Events validate against the `1.0` activity-event schema and carry canonical ids from the documented
  length-prefixed layout. [`ids.py`](../eval/synthetic/src/timelex_goldset/ids.py) is pinned to the
  same golden value as `timelex-types`, so Python and Rust cannot drift silently. Every mail and
  calendar event names the message or meeting it came from, in both directions.
- Labels are separate from observations and state `expectedStorage`, `expectedOutcome`, client,
  matter, billability, evidence and every honest candidate matter. The corpus contains 109
  `auto_post`, 82 `review` (70 ambiguous across open matters, plus closed-matter signals), 17
  `not_billable` and 12 `excluded` events. Excluded-domain signals are present because a connector
  would see them, and labelled so capture must drop them. Every event is metadata-only.
- [ADR-0025](adr/0025-synthetic-gold-set.md) records the determinism, synthetic-only and
  label-first choices and their limits. [`.gitattributes`](../.gitattributes) pins the corpus to LF.
  CI gains a gold-set job running ruff, pytest and the drift check. TASKS, ARCHITECTURE, INDEX,
  [`eval/README.md`](../eval/README.md), the root README and the package README are updated.

**Verification**
- 29 tests pass: canonical id and boundary shift; sub-seconds and offsets name the same instant; a
  naive datetime is refused; same seed gives identical bytes; different seed changes the tenant and
  every id; every event validates against the schema; every id recomputes; intervals are positive and
  capture follows the end; labels cover every event once and in order; threads and meetings are
  referentially complete; excluded domains are never labelled stored; candidate sets are honest and
  client-only signals list every open matter; closed matters expect review; unassigned signals are
  stored but not billable; addresses stay on reserved domains; no credential-shaped text; the
  committed corpus matches the generator; the manifest digests, sizes and all counters match; and
  tampering or a missing file is reported.
- CPython 3.12 is pinned in `eval/synthetic/.python-version` and `uv sync --locked` is used in CI,
  because the standard library guarantees the `random()` stream across versions but not the
  higher-level helpers the generator uses.
- `ruff check` and `ruff format --check` pass; `python -m timelex_goldset check --corpus corpus`
  prints `corpus is current`. The corpus is 220 events across six files, about 380 KB in total.
- Docs links are checked separately below.

**Decisions and why**
- **Deterministic and committed, not generated per test run.** A committed corpus with a manifest
  gives a reviewable diff and a CI check; a random fixture would hide changes and failures.
- **Labels separate from observations.** A label can say "drop this", "this is ambiguous" or "this
  is stored but not billable", which a single correct-matter field cannot express.
- **Real contract, real ids.** The corpus uses the activity-event schema and the canonical id, so it
  rehearses the capture path instead of a parallel test shape.
- **Excluded signals are included.** Omitting them would make it impossible to prove capture drops
  them; the label and README state the rule so a harness cannot mistake them for stored work.
- **No bodies yet.** Content is blocked behind the redaction path, so the corpus measures metadata
  stages honestly and the `history` evidence is reserved rather than faked.

**Next:** P3-03 segmentation and P3-04 minimal matters are the next unblocked build tasks and the
natural consumers of this corpus. P3-11 will add the harness that applies `expectedStorage` and
scores the pipeline. The redaction worker queue and container remain the second half of P2-07, best
started when a connector can produce content. P2-03 still needs a real Microsoft 365 test tenant.

## 2026-11-02: P2-10 — a firm can read and export what this build does with its data

**Trigger:** ADR-0007 point 7 requires every tenant to have an exportable record of what data goes to
which processor for the firm's POPIA documentation. P2-07 also promised that adding an exclusion would
not clean the past, so that warning needed a surface the firm would actually see. An administrator had
routes for exclusions but no page, and no committed description of the build's data flows.

**Done**
- [`contracts/privacy/data-flow.v1.json`](../contracts/privacy/data-flow.v1.json) is a versioned (1.0) technical register: scope, disclaimer, capture defaults, 8 data categories, 8 flows, 5 processors, the exclusion scope and a retention note. Every flow and processor carries `active`, `development_only` or `not_active`, so an unbuilt connector is visible rather than omitted. The register states that content is not captured by default, that redaction is required before content, that manual entries keep only the title, that domain exclusions run before storage, that exclusions do not apply to existing events and that no automatic retention is configured.
- [`gateway`](../services/gateway) serves the committed file at open `GET /v1/privacy/data-flow` with `no-store`, `nosniff` and `application/json`, and serves the admin page at `GET /privacy` through the same strict-CSP helper as `/capture` ([`lib.rs`](../services/gateway/src/lib.rs)). [`privacy.html`](../services/gateway/assets/privacy.html) renders the register, offers the JSON download, and lists, adds and removes exclusions with the existing routes. The page keeps the API key in memory, uses `textContent` only, loads no external asset and has no em dashes.
- [`gateway.v1.yaml`](../contracts/openapi/gateway.v1.yaml) becomes 0.7.0 with the data-flow path and a `DataFlowRegister` schema; the SDK is regenerated and the drift check passes. [ADR-0024](adr/0024-data-flow-register-and-disclosure.md) records the choices. Docs updated: TASKS, ARCHITECTURE, INDEX, gateway README and the root pre-alpha note.
- Tests: 2 new gateway unit tests. One serves `/privacy` and checks its headers, its links, the `settings:manage` note and the absence of storage, `innerHTML` and external URLs. The other serves the register and checks version 1.0, the honest defaults, `appliesToExistingEvents: false`, the non-empty tables, the known flow statuses and the absence of credential-shaped strings. Gateway counts: 28 unit, 2 contract, 31 routing, 9 end-to-end.
- **Line-ending repair:** the rebuilt compose image refused the already-applied migrations because Git had checked the `.sql` files out with CRLF while sqlx had recorded the LF checksums. The recorded values matched the CRLF bytes exactly for identity 1, vault 1 and capture 1, proving the content had not changed. [`.gitattributes`](../.gitattributes) now pins `*.sql` to LF, the worktree files were rewritten from their index blobs, and the three checksum rows were corrected in place. No schema or data was dropped.
- Verification: fmt clean, clippy `-D warnings` clean, the full workspace green with the test database, `npm run build`, `npm test` and `npm run check:sdk` green, and 520 links across 75 Markdown files resolve. The compose stack rebuilt and came up healthy. A Playwright run on `/privacy` rendered the register, connected a fresh admin key, added `Excluded.Example.TEST.` (stored as `excluded.example.test` with its reason), added the same domain again (200, "already configured", still one row), added a client `CL-2026-0042` (case kept), removed both, and ended at "No exclusions configured" with no console errors and no web storage.

**Decisions and why**
- **One static register, not a generated per-tenant document.** The flows are properties of the build, not the tenant, and per-firm processor choices do not exist yet. A committed file with a version can be reviewed and diffed; a generated document would imply tailoring the build cannot honour.
- **Inactive things are listed and labelled.** Omitting a processor is indistinguishable from not disclosing it. `not_active` makes the gap explicit for the firm and for the next engineer.
- **Facts, not promises, and a disclaimer.** The register repeats what a reader can check in the code and says plainly that it is not legal advice. It also states that exclusions do not clean the past, so the firm's own notice cannot accidentally claim otherwise.
- **The admin page has no privileged path.** It calls the same public routes as any client, so capture still enforces `settings:manage` and there is no second authorization model to get wrong. The key stays in memory, following the P2-08 decision after CodeQL flagged storage.
- **The migration fix corrects bytes, not history.** The checksums were rewritten only after proving the files differed solely by line endings, and neither schemas nor rows were touched.

**Next:** a signed per-user consent artefact and a per-firm layer for processor and region choices remain open under P2-10. The redaction worker queue and container are the second half of P2-07. P2-03 still needs a real Microsoft 365 test tenant. P1-12 remains a repository setting.

## 2026-11-02: P2-07 — privacy exclusions act before an event exists

**Trigger:** a firm must be able to say what must never be captured, and the promise has to be true before the signal becomes history. An activity event is immutable, is copied into the outbox and will be cited by time entries and bills, so filtering after storage would hide a row that already exists rather than prevent capture. The filter also has to sit below the gateway, because connectors call the store directly, and it has to stop unredacted content entering the event store before the redaction worker is deployed.

**Done**
- [`services/capture-hub/migrations/0002_exclusions.sql`](../services/capture-hub/migrations/0002_exclusions.sql) adds `capture.exclusions`: tenant-scoped and RLS-forced, primary key `(tenant_id, kind, value)`, checks for the three kinds, domain shape, value and reason lengths, plain-text reasons and the `us_…` creator id. `svc_capture` holds `SELECT`, `INSERT` and `DELETE` only; a reason cannot be rewritten in place.
- [`exclusions.rs`](../services/capture-hub/src/exclusions.rs) holds the pure rules and 8 tests. Domains are trimmed, lose one leading `@` and one trailing dot, are lower-cased and must be valid ASCII labels of at most 253 characters. Matching is label-aware and covers subdomains, so `example.com` catches `mail.example.com` but not `notexample.com`. Participant domains come from `domain`, falling back to the domain part of `address`; a malformed domain is skipped rather than stalling a connector. Client and matter identifiers keep their case and allow letters, digits, `.`, `-`, `_` and `:`; reasons are trimmed, at most 500 characters and free of control characters.
- [`store.rs`](../services/capture-hub/src/store.rs) enforces the list inside the tenant write transaction. `record` and `record_page` load the firm's domains once, test each event before the insert and skip an excluded one, so no row and no `activity.recorded` outbox row are written. A single excluded ingest returns `Excluded` and the HTTP layer answers 200 `created: false`, exactly like a duplicate. A connector page counts the event as `excluded`, stores the rest and still commits its cursor, so one privacy rule cannot stop capture. `add_exclusion`, `list_exclusions` and `remove_exclusion` all require `settings:manage`; adding is idempotent, keeps the first reason and creation time, and removing an absent value reports not found.
- `validate` now enforces the metadata-only gate: a `contentRef` only with `contentCaptured` and `redactionApplied`, and a reference of 1 to 512 characters. A reference without content, capture without redaction, redaction without capture and capture without a reference are all refused before storage.
- [`http.rs`](../services/capture-hub/src/http.rs) serves `GET`/`POST /v1/exclusions` and `DELETE /v1/exclusions/{kind}/{value}`; [`gateway`](../services/gateway) exposes the same under `/v1/privacy/exclusions`, validates the kind and a conservative value shape before calling capture, and mints a token for capture's audience. Contracts become capture 0.3.0 and gateway 0.6.0, and the SDK is regenerated.
- Tests: 8 exclusion-unit, 23 store and 24 HTTP tests in capture-hub, plus 2 gateway routing tests (the forward with kind and value, and a malformed value that never reaches capture). The exclusion tests bite: setting both enforcement conditions to `false` made the store tests see `Created` instead of `Excluded` and a page report `created: 2, excluded: 0`; both were restored.
- Verification: fmt clean, clippy `-D warnings` clean, the full workspace green, `npm run check:sdk` matching 4 contracts. The compose image rebuilt, `capture-migrate` applied version 2 and the dev database shows the table, checks and forced RLS. Through the public gateway routes a fresh admin key added ` Example.COM. ` (201, stored as `example.com`), repeated it (200, `created: false`), listed it with its reason, got 400 for an unknown kind, removed it (204), removed it again (404) and listed an empty set.

**Decisions and why**
- **The filter runs before the insert, inside the write transaction.** A downstream filter leaves an immutable event that may already have been read; deleting it edits history and is refused by the write-once grant. Running below the gateway means connectors and public ingest get the same rule.
- **Domains cover subdomains and ignore case.** An exact-match filter is bypassed by `mail.example.com`; a case-sensitive one by `Example.COM`. Label-aware matching closes both without matching lookalike names.
- **An excluded single ingest looks like a duplicate.** A distinct status would reveal the firm's policy to any client that can post events. `created: false` gives the sender what it needs to stop retrying and nothing more.
- **A page's cursor still advances.** Holding the cursor because one event was excluded would re-read the same page forever, turning a privacy rule into an outage. The excluded count is the evidence that the rule fired.
- **Client and matter are recorded now, enforced in Phase 3.** A raw signal names neither, so claiming enforcement would be a false guarantee. The table is ready for attribution to consult.
- **Adding is idempotent and first-reason-wins.** Two admins adding the same domain should not produce a race or rewrite the audit trail. There is no `UPDATE` grant; removing and re-adding is the explicit way to change a reason.
- **The metadata gate is a storage rule, not a connector convention.** Until the redaction worker is wired, it is what makes "metadata only" true for every sender, including a buggy one.

**Next:** the second half of P2-07 is the redaction worker queue and container around the P0-08 module. P2-03 still needs a real Microsoft 365 test tenant for its 14-day acceptance; without one, the next verifiable items are the agent-facing pieces of capture or the gap detector's inputs (P2-09). P1-12 remains a repository setting.

## 2026-11-02: P2-08 — a person can record work by hand

**Trigger:** connectors only see what mail, calendars and a desktop agent see. A call taken in a corridor, a meeting off the network or a piece of work done away from the machine leaves no signal, so the day would always have a hole. P2-08 gives a person a way to record that work as a first-class activity event, and it gives the platform an idempotent client path to prove the capture contract end to end before the first real connector exists.

**Done**
- [`services/capture-hub/src/manual.rs`](../services/capture-hub/src/manual.rs) turns an entry into a canonical activity event. The request is `{entryId, kind, startedAt, endedAt, description}` with `kind` one of `call`, `meeting` or `manual_entry`. The event source is `manual`, the source id is the `entryId` UUID, and the canonical id is computed by the same rule as every other event. `description` is whitespace-collapsed into `subject.title` and capped at 500 characters; a blank description stores no subject. Privacy is `contentCaptured: false`, `redactionApplied: false`. Every rejection happens before storage: end before start, an `entryId` that is not a UUID, an unknown field or an oversized description is a 400.
- [`http.rs`](../services/capture-hub/src/http.rs) adds `POST /v1/entries` behind a principal token with `capture:write`, builds the event with an injected clock and reuses the same store path as `POST /v1/events`: 201 with `created: true` on a first insert, 200 with `created: false` when the retry lands on the existing row. The event and its `activity.recorded` outbox row commit together.
- [`gateway`](../services/gateway) exposes `POST /v1/capture/entries` and forwards only the body to the private `/v1/entries`; every caller-chosen header is stripped as on the other routes. The same router serves a self-contained mobile page at `/capture`: call, meeting or note, start and end times defaulting to the last 15 minutes, a 500-character description with a counter, and a collapsible connection panel. The page sends an `entryId`, keeps it across a failed save so a retry is idempotent, and holds the API key in page memory only, never in web storage. It is served with a strict CSP, `nosniff`, `no-referrer` and `no-store`, with no third-party assets and no em dashes.
- [`capture.v1.yaml`](../contracts/openapi/capture.v1.yaml) becomes 0.2.0 and [`gateway.v1.yaml`](../contracts/openapi/gateway.v1.yaml) 0.5.0; the SDK is regenerated and the drift check passes. [ADR-0022](adr/0022-manual-quick-capture.md) records the five decisions. Compose routes the public entry path to `capture` and makes the gateway wait for a healthy capture service.
- Tests: 9 manual-normalisation tests in capture-hub, 7 new HTTP tests (the entry stored as an event, a retry stored once, missing permission, end before start, an unknown kind, an unknown field and an oversized description), and one gateway routing test that proves the entry goes to capture rather than identity with the body intact. Gateway counts: 26 unit, 2 contract, 29 routing and 9 end-to-end. The retry test bites: making the source id a random UUID instead of the entry id makes a second delivery create a second event, and it failed as expected before the code was restored.
- The stack smoke test found a policy bug the unit tests could not: `capture:write` belonged only to the machine `agent` role, so the initial admin key and every fee earner got a 403 from the page. The role matrix now grants it to `fee_earner` and `admin` (not `billing` or `auditor`), pinned by a new test that failed first. Manual capture records an observed signal, not a billable time entry, so it does not imply `time:write`.
- Verification: fmt clean, clippy `-D warnings` clean, the full workspace green, `npm run check:sdk` passing, and the compose stack healthy. The dev volume held a stale checksum for the capture migration from the P2-02 branch; the `capture` schema was empty (0 events, 0 cursors), so only that schema was dropped and the idempotent bootstrap recreated it without touching identity or vault data. The API smoke test returned 201 then 200 with one stored row, and a Playwright run at 390x844 saved a meeting entry (confirmed in Postgres) and showed both the missing-key and rejected-key messages.
- CodeQL flagged the first version of the page for storing the API key in `sessionStorage`. The page now keeps it in memory only, and a gateway test asserts the asset contains neither `sessionStorage` nor `localStorage`, so the property survives future edits.

**Decisions and why**
- **A dedicated `/v1/entries` resource, not a hand-built event.** A browser should not have to know the event contract, compute a canonical id or get the source spelling right. The server derives all of that, and the public route cannot be used to inject an arbitrary event.
- **The entry id is the idempotency key.** A mobile page on a weak connection will retry; the same `entryId` must land on the same row. The page keeps the id across failures and the server uses it as `source.sourceId`, so the existing database unique constraints do the deduplication.
- **Three kinds only.** `call`, `meeting` and `manual_entry` cover what a person records by hand without inventing a taxonomy the attribution stage cannot use yet. `email_sent` remains a connector fact.
- **Description is a title, not content.** The text becomes `subject.title` and the event says content was not captured. Storing a transcript or a note body would break the metadata-only default (ADR-0006) before the privacy pipeline exists (P2-07).
- **The page is served by the gateway, same origin.** A separate app would need CORS, a second origin in the CSP and another deployable for one form. Same-origin removes the class of mistakes, and a strict CSP keeps the page from loading anything it did not ship with.
- **The key lives in page memory.** It never enters web storage, so a reload asks for it again, and it is never in the URL or in a cookie. The alternative, an httpOnly cookie, needs a browser session flow that identity does not have yet; `sessionStorage` would survive a reload but is still readable by any script on the origin, so the page does not use it.

**Next:** P2-03, the Microsoft 365 mail connector, which needs a real test tenant for its 14-day acceptance; until one exists, the next verifiable items are the gap detector's inputs (P2-09) or privacy exclusions (P2-07). P1-12 remains branch protection and needs a shared repository setting.

## 2026-11-01: P2-02 — the capture hub stores each signal once

**Trigger:** every capture source delivers at least once, so the same email, meeting or focus window arrives twice after a retry, a restart or a replay. P2-01 made the event contract immutable and versioned; P2-02 had to make the store refuse the second copy, including across instances and deploys, and give connectors a cursor that cannot skip an event.

**Done**
- [`crates/timelex-types`](../crates/timelex-types/src/lib.rs) gains the canonical activity-event id: `ae_` plus the first 16 bytes of a length-prefixed SHA-256 over the firm, `source.system`, `source.sourceId` and whole-second `startedAt`, under a `timelex/activity-event/id/v1\0` domain. Kind, actor, subject, capture time and delivery time are outside the rule. `supports_activity_schema_version` accepts every `1.x`. 9 tests, including a golden digest computed independently: changing any of the four facts changes the id; changing anything else does not; a boundary shifted between two fields cannot collide; sub-second precision truncates.
- The rule is published in [`contracts/events/README.md`](../contracts/events/README.md) and the id description in [`activity-event.schema.json`](../contracts/events/activity-event.schema.json) now points at it.
- [`services/capture-hub`](../services/capture-hub) becomes a service: migration `capture.activity_events` (primary key `(tenant_id, id)`, unique signal key, enum and size checks, write-once grants) and `capture.connector_cursors`, both under RLS. The store ingests with `ON CONFLICT (tenant_id, id) DO NOTHING` and appends `activity.recorded` to the outbox only when a row was actually inserted, in the same tenant transaction.
- [`connector.rs`](../services/capture-hub/src/connector.rs) adds the `Connector` trait (`fetch(cursor, limit)` → page plus opaque `next`), the `Runner` that drains every connector from its stored cursor, and `replay`, which reads from the beginning through the same idempotent path without touching the stored cursor. A page and its cursor commit together; an oversized page or a `next` that does not advance is refused before anything is written.
- [`http.rs`](../services/capture-hub/src/http.rs) adds `POST /v1/events`: a principal token with `capture:write`, the event's firm and actor must be the caller's, the id is recomputed and a mismatch is a 400, and the answer is 201 new or 200 existing with `created`. Bodies over 64 KiB are 413; problems are RFC 9457.
- [`contracts/openapi/capture.v1.yaml`](../contracts/openapi/capture.v1.yaml) and [ADR-0021](adr/0021-idempotent-capture-and-replay.md) document the surface, the id rule, the cursor rule and replay. The service is in the [`Dockerfile`](../Dockerfile) and the compose stack (migrations and a healthchecked `capture` container).
- Verifying the stack exposed two latent bugs. The `Dockerfile` copied `crates/` and `services/` but not `tools/`, so `--locked` workspace resolution had been broken since P1-11 added `tools/new-service` as a member; the image now copies that directory (and `.dockerignore` allows it while still excluding the SDK generator). The Postgres bootstrap ran only when `pgdata` was empty, so an existing dev volume would never gain `svc_capture`; a one-shot `bootstrap` container now re-applies the idempotent files, including a new `svc_capture LOGIN` line, on every `up`. Verified on the existing volume: all six containers healthy, `capture` listening on `:8084` and relaying the outbox, and `capture.activity_events`, `capture.connector_cursors` and `svc_capture` present. Also smoke-tested `migrate`, `serve` and `/health` against a scratch database.
- Tests: 27 in `timelex-types`; 14 store, 9 runner and 13 HTTP database tests. The acceptance test delivers the same event twice and asserts one row and one outbox event. I made it bite twice on purpose: a conflict clause that touches the row fails with the write-once grant, and an early-return mutation fails on `created`. Both were restored.

**Decisions and why**
- **The id is computed from the signal, so deduplication needs no memory.** Any in-memory or cache-based set forgets on restart and differs per instance. A database primary key is the one place a second delivery meets the first in every deployment shape.
- **The rule excludes kind, actor and capture time.** A corrected label, a late capture or a redaction pass must not turn one signal into two events. The four inputs are the ones that say *which signal this is*.
- **The receiver recomputes the id.** A sender-supplied key would let a buggy connector force duplicates by inventing ids; recomputing makes the id a checkable claim and a mismatch a 400.
- **Length prefixes and a NUL-terminated domain.** Concatenation alone lets `ab`+`c` collide with `a`+`bc`; the layout is part of the contract because every future `1.x` reader must compute the same id.
- **Events stay write-once.** The service role has no `UPDATE` or `DELETE`; a correction is a new event (ADR-0006). The duplicate path must therefore change nothing, which is what the outbox guard enforces.
- **Page and cursor in one transaction.** Storing first with no cursor is merely re-reading; moving the cursor first loses events. One transaction removes the choice.
- **Replay does not move the cursor.** A replay after a fix fills gaps, while a scheduled run continues from where it was; moving the cursor would make the next run re-read work replay had already covered.
- **The gateway is not wired yet.** There is no public caller until quick capture (P2-08); the contract says the interface is private, and adding a gateway route with no client would be surface without a user.

**Next:** P2-03, the Microsoft 365 mail connector (admin consent, subscriptions, renewal, lifecycle), as the first real `Connector` implementation. P1-12 remains branch protection and needs a shared repository setting.

## 2026-11-01: P2-01 — event contract v1, and the versioning rules that make it true

**Trigger:** the activity-event schema and its Rust mirror existed since the walking skeleton, but P2-01 asked for versioning rules and a backward-compatibility test. The old contract promised that "additive changes bump the minor" while `additionalProperties: false` and serde's `deny_unknown_fields` made every reader reject a newer minor. During the first rolling deploy, an old consumer would have thrown away valid events. A promise the schema contradicts is worse than no promise.

**Done**
- [`contracts/events/README.md`](../contracts/events/README.md) sets the v1 rules: only optional properties may be added in a minor; readers ignore unknown properties at every level; required properties, types, meanings and constraints are fixed; enum vocabularies are fixed; events are immutable; nothing reaches the stream invalid; each release freezes an example; and the tests prove both directions. A breaking change is a new major file (`activity-event.v2.schema.json`) while v1 stays readable for stored events.
- The released examples are frozen as [`activity-event.v1.0.json`](../contracts/examples/activity-event.v1.0.json) and [`secret-event.v1.0.json`](../contracts/examples/secret-event.v1.0.json). `examples/<name>.json` remains the current example used by the docs and the SDK.
- Both [event schemas](../contracts/events/activity-event.schema.json) drop `additionalProperties: false` at every object level, and the matching Rust types drop `deny_unknown_fields`. Unknown properties are ignored on read and dropped on write; they are never guessed at or copied forward.
- [`crates/timelex-types`](../crates/timelex-types/src/lib.rs) grows from 7 to 17 tests. New ones: a frozen 1.0 event still validates and deserialises; a 1.1 event with unknown fields at the top level, in `source`, in `subject`, in a participant and in `privacy` validates and deserialises, and the known fields survive; the same for secret events. Missing required fields, a bad `kind` and a `2.0` major are still rejected.
- The SDK types are regenerated. The generated `ActivityEvent` now carries index signatures, which is how the TypeScript contract expresses the same tolerance.

**Decisions and why**
- **Readers tolerate, writers do not add required fields.** The outbox delivers at least once and services roll out one at a time, so an old consumer must survive a newer minor. Tolerating unknown fields is the only way the minor rule is true in practice. A producer still cannot add a required field, because a missing one breaks old readers.
- **Enum values are a new major, not a new minor.** A reader that meets an unknown `kind` or `system` cannot interpret it honestly; silently skipping would hide billable work. The v1 vocabulary already names every planned capture source, so v2 is a deliberate cost when a genuinely new meaning arrives.
- **Frozen examples rather than a second schema.** The test reads the bytes that were actually released, so editing history to fit a new schema fails CI. A consumer-only schema would test a document nobody ships.
- **No producer-side strict copy of the schema.** Producers construct typed events, so they cannot emit unknown fields; a second schema would only add a file to keep in step.

**Next:** P2-02, the `capture-hub` framework (connector trait, idempotent ingest, replay). P1-12 remains branch protection and needs a shared repository setting.

## 2026-11-01: P1-13 increment 2 — the host leases its key, and the vault audits the use

**Trigger:** the model loop and the MCP client were done, but the host still had no way to obtain the firm's Anthropic key. ADR-0018 point 8 said the key is a per-run lease, and the vault's ADR-0004 lease had been waiting for its first caller. The blocker was identity: a background host has no user session, and ADR-0012 makes the gateway the only issuer of user tokens.

**Done**
- [`crates/timelex-auth/src/workload.rs`](../crates/timelex-auth/src/workload.rs) adds workload identity ([ADR-0020](adr/0020-workload-identity-and-leases.md)). `WorkloadSigner` mints `iss: timelex-<service>`, `aud`, `kind: workload`, `svc`, `tid`, `sub`, `purpose`; `WorkloadVerifier` trusts an explicit `service:kid=base64` map with globally unique key ids. Both reuse the gateway's signature, lifetime, skew and clock rules. The two verifiers are disjoint: a principal token is not a workload token and a workload token with `key` or `perms` is malformed. 13 tests, including a stolen key that signs a sibling service's name.
- The vault store gains `lease(LeaseActor, SecretKind)` ([`services/vault/src/store.rs`](../services/vault/src/store.rs)): one tenant transaction, `FOR UPDATE` on the live row, `last_used_at`, a `used` audit entry (`lease <service> <purpose>`, actor the initiating user), no outbox event. `LEASE_SECONDS` is 60. Four database tests: the value and expiry, revocation, another firm's invisibility, and a replacement's value.
- The HTTP route `POST /internal/v1/secrets/{kind}/lease` ([`services/vault/src/http.rs`](../services/vault/src/http.rs)) takes a `Workload` extractor and a purpose→kind map (`model_run` → `anthropic_api_key`) checked before any lookup, so a wrong purpose cannot probe which kinds exist. `TIMELEX_WORKLOAD_PUBLIC_KEYS` is optional; absent, every lease is 401. Four HTTP tests cover the round trip and `lastUsedAt`, every rejected token shape plus the fail-closed vault, the purpose check, and firm/revoked isolation.
- [`services/mcp-host/src/vault.rs`](../services/mcp-host/src/vault.rs) adds `VaultKeySource`, `LeasePurpose::ModelRun`, `Lease` and `LeaseError`. It signs per lease, posts with no body, accepts only the kind it asked for, parses the value through `SecretValue`, redacts it in `Debug`, and keeps the vault's text out of every error. Five tests against a loopback stub that verifies the token exactly as the real vault does.
- Contracts and docs: [`vault.internal.v1.yaml`](../contracts/openapi/vault.internal.v1.yaml), a workload-token section in `contracts/internal-auth.md`, ADR-0020 and INDEX, TASKS (P1-05 and P1-13), ARCHITECTURE, both service READMEs and this entry.

**Decisions and why**
- **Per-service keys, not a gateway-minted token.** The gateway would have to sign for a service with no user request behind it, which is the confused deputy ADR-0012 exists to prevent. A shared secret was already rejected there, and mTLS carries no firm, user or purpose.
- **Lease by kind, not by secret id.** Leasing by id would need a metadata call and `settings:manage`, an admin permission a background host should not hold. The purpose table gives it exactly one kind.
- **The purpose is checked before the lookup.** Otherwise a compromised host could use 403/404 to learn which kinds a firm has stored.
- **No event on a lease.** An access is not a change; a busy host would bury the activity feed. The audit chain and `lastUsedAt` are the record.
- **Expiry is a contract, not a recall.** The vault hands over plaintext and keeps no revocable copy, so 60 seconds plus the caller's obligation to drop the value is the honest rule. A server-side revocation list would imply a recall that does not exist.
- **Fail closed.** No configured workload key means no lease, and a malformed value stops startup. There is no development bypass.

**Next:** the `host` schema and the HTTP workflow routes (P3-06). P1-12 is still branch protection and needs a shared repository setting.

## 2026-11-01: P1-13 increment 3 — the host can call an MCP server

**Trigger:** the model loop could spend a firm's Rand, but nothing connected it to the tools a workflow needs. P1-08 verified the server side of `rmcp`; the client side was the missing half of P1-13, and the SDK's client defaults include a legacy fallback and session recovery that TimeLex must never use.

**Done**
- [`crates/timelex-mcp/src/client.rs`](../crates/timelex-mcp/src/client.rs) adds `ClientTarget`, `McpClient`, `TargetError` and `ClientError` ([ADR-0019](adr/0019-mcp-client-rules.md)). A target is validated before a connection exists: absolute URL, `https` required, plain `http` only for `127.0.0.0/8`, `::1` or `localhost`, and no username, password or fragment.
- `McpClient::connect` uses `ClientLifecycleMode::Discover` with `[2026-07-28]` only. It never sends `initialize` and never falls back: a legacy-only server fails `Startup`, an older-only server fails `Protocol`. The transport is stateless and `reinit_on_expired_session` is off, so a 404 is surfaced rather than hidden by a re-handshake.
- The bearer token is the only credential path: validated as printable ASCII, stored in a zeroizing wrapper inside the target, sent only as `Authorization: Bearer …`, and redacted from `Debug`. No URL carries it.
- `list_tools` and `call_tool` return finished results. `call_tool_once` is used deliberately; `input_required` and task replies are refused as `InputRequired` and `Task`, because the host has no MRTR input source and no task poller yet.
- `ClientError` is closed and quiet: `Startup`, `Protocol`, `Transport`, `Rpc { code }`, `InputRequired`, `Task`, `Closed`. The server's message, body and URL never enter the error.
- Tests: 9 in [`tests/client.rs`](../crates/timelex-mcp/tests/client.rs) against the in-repo stateless server and two stubs. They prove discovery, `tools/list`, `tools/call`, no session header, the bearer on every request with the token in no other header, a legacy-only peer refused without `initialize`, and an older-only peer refused. Together with the 23 existing server probes that is 32 tests in the crate.
- Dependencies: `url` for URL parsing and `zeroize` for the token; `rmcp` gains the `client`, `transport-streamable-http-client-reqwest` and `reqwest` (rustls) features. The exact `=3.6.0` pin is unchanged.
- Docs: ADR-0019, INDEX, TASKS, ARCHITECTURE, the root README.

**Decisions and why**
- **Discovery only, no `Auto`.** `Auto` probes and then silently falls back to `initialize` after ten seconds or a legacy error. A server must never negotiate TimeLex down.
- **No session recovery.** TimeLex servers are stateless and never issue a session; a 404 means a wrong route, and re-handshaking would turn a routing bug into a working-but-different connection.
- **Plain HTTP only on loopback.** A bearer token and tool arguments are not something a "private network" assumption makes safe.
- **Refuse MRTR and tasks for now.** Both need workflow state and a human approval surface owned by P3-06/P2-11. An explicit refusal is testable; a hang is not.
- **Keep only the JSON-RPC code.** Remote text in an error is remote text in a log and possibly in a prompt, the same rule ADR-0018 applies to providers.

**Next:** the vault lease (first caller of `/internal/v1/...`, with audit and contract tests) and a lease-backed key source; then the `host` schema and workflow routes (P3-06). P1-12 is still branch protection and needs a shared repository setting.

## 2026-11-01: P1-13 increment 1 — the model loop, and a Rand cap that really stops

**Trigger:** `mcp-host` is the only Phase 1 component with no code, and it is where a firm's own Anthropic money is spent. Before any workflow can run, the loop needs a provider boundary, exact cost accounting and a ladder that cannot spend past its authorization.

**Done**
- [`services/mcp-host`](../services/mcp-host) is a new workspace library ([ADR-0018](adr/0018-model-loop-money-and-failure-rules.md)). `ModelProvider` has four errors chosen for the next action: `ModelUnavailable` (next rung), `KeyRejected` (stop; all rungs share the key), `Unreachable` (stop; a rate limit is not fixed by a smaller model) and `Unusable` (stop).
- `AnthropicProvider` sends one `POST /v1/messages` with the leased key in `x-api-key` only. It maps `401` to a key failure, `400`/`403`/`404` to model-unavailable, and `408`/`429`/`5xx`/transport to unreachable. Provider messages, bodies and status codes never enter an error: a test checks that the provider's words do not appear in the error's `Debug` or `Display`.
- Money is integer micro-rand. `PriceList::published()` holds the research 04 §7 rates at R18/USD (Haiku R1.80/R9, Sonnet R36/R180, Opus R72/R360 per million input/output), with cache reads at 0.1x and writes at 1.25x input. 2 000 input and 400 output tokens cost R0.0072 on Haiku, and a test pins it.
- The ladder is Haiku → Sonnet → Opus → Fable, where Fable needs both the tenant's 30-day-retention flag and a configured price. With neither, Opus is the top and escalation is `needs_review`.
- The loop checks the cap before each call and after each reply. A R0.005 cap aborts after a R0.0072 call and never makes the second; a zero cap makes none. `max_steps` is a second ceiling. Every run ends `done` or `needs_review` with a reason (`budget_exceeded`, `escalated_to_human`, `no_model_available`, `key_rejected`, `provider_unreachable`, `unusable_reply`, `step_limit`).
- Tests: 2 unit, 14 loop tests over a scripted mock provider and 9 adapter tests over a local Axum stub, plus a doctest. No test reaches the real provider.
- Docs: ADR-0018, INDEX, TASKS (P1-13 now 🟡), ARCHITECTURE, the root README and this service's README.

**Decisions and why**
- **Integer micro-rand, never floats.** A cap test must be exact; two equal runs cannot differ in the last digits.
- **Prices are deployment configuration, not tenant data.** A tenant controls its cap, not what a token costs. A stale or malicious price row could make a call free.
- **No Fable rung without a price.** The tenant flag alone would allow an uncapped call; our research has no Fable rate, so it stays off until one is configured.
- **The cap stops even an accepted reply once spend is over.** The run has exceeded its authorization; a human decides what to do with the output.
- **No retries in the adapter.** A retry with its own backoff would spend money the caller's budget cannot see. Backoff goes above, against the same budget.
- **A library first.** `/v1/workflows` and `/v1/runs` need the workflow shape, which P3-06 owns; inventing the contract now would produce a surface with no users.

**Next:** the vault lease (first caller of `/internal/v1/...`, with audit and contract tests) and a lease-backed key source; then the `rmcp` client probes; then the `host` schema and workflow routes. P1-12 is still branch protection and needs a shared repository setting.

## 2026-11-01: P1-11 new-service generator — the frame arrives with the service
**Trigger:** identity, vault and gateway each carry the same operational frame by hand: internal-token check, tenant transaction, `/health`, problem-detail errors, telemetry, a schema with RLS and grants, and a workspace member. The copies already differ in details, and Phase 2 adds several services. The fourth copy is the one that forgets a grant or accepts a tenant id from the request.

**Done**
- [`tools/new-service`](../tools/new-service) is a dependency-free workspace binary. Templates are embedded with `include_str!`, so the tool ships with the repository. `generate(root, name)` writes `services/<name>` and patches exactly two existing files: the root `Cargo.toml` `members` list and `deploy/postgres/bootstrap/004_provision_services.sql`. Both edits are idempotent, and an existing service directory is refused rather than merged.
- The name rule is the one `platform.provision_service` enforces: two to 31 characters, a lowercase letter first, then lowercase letters, digits or underscores, refusing `platform`, `public` and `pg_%`. It becomes the directory, the crate `timelex-<name>-service`, the schema and the role `svc_<name>`.
- The generated service has the gateway-token extractor with the three caller kinds (401 for anything unverifiable, 403 for a valid token of the wrong kind), tenant transactions under row-level security, `/health`, telemetry (`init` and `instrument`), RFC 9457 problem bodies, a 64 KiB body limit and no echoing of input. It owns one tenant-scoped `items` table with the standard RLS policy and explicit grants, plus 5 database tests: health without credentials, 401 and 403, create and list, cross-firm isolation and bad input.
- CLI: `cargo run -p timelex-new-service -- <name>` prints what it created and patched and names the next manual steps (contract, gateway route, compose).
- CI: the Rust job now generates `example`, runs `cargo fmt --check`, `cargo clippy -D warnings` and the generated database tests, then restores the two patched files and removes the directory.
- Docs: [ADR-0017](adr/0017-new-service-generator.md), the tool's own README, TASKS, INDEX, ARCHITECTURE, the root README and the Postgres readme.
- Verified: 8 generator tests pass (names, all nine files and their content, token substitution, refusal to overwrite, idempotent patches, missing members list). Generating `example` and running fmt, clippy and the 5 database tests against a throwaway Postgres passed; the scratch directory and patches were then removed.

**Decisions and why**
- **Generate inside the workspace, test in CI.** A boilerplate repository drifts silently when the workspace's lint settings or dependency versions change. A generated member checked on every CI run makes that drift a failed build.
- **Patch two files only.** Contracts, gateway routes and compose services need names, scopes and decisions the tool cannot infer; a wrong guess looks finished. The generated README lists them as next steps.
- **Refuse existing directories.** A generator that merges with someone's work is one bad edit away from destroying it.
- **One honest example, not a product feature.** `items` proves the frame end to end and is labelled as an example in the code and the README. Every generated test asserts something that would break if tenancy or authentication regressed.
- **Embedded templates, no dependencies.** The template is the reference for the frame; keeping it in-repo means the tool cannot fetch a stale one, and the binary adds no supply-chain surface.

**Next:** P1-13 (`mcp-host` skeleton: `ModelProvider` trait, Anthropic adapter, budgets, mock-provider test) is the remaining Phase 1 code gap; P1-12 is branch protection and needs a shared repository setting. When a generated service first needs a rule of its own, decide whether the auth and problem-detail frame should move into a shared crate rather than a fifth copy.

## 2026-11-01: P1-10 telemetry kit — one trace from the caller to the service
**Trigger:** a request that crossed the gateway into identity or the vault had no shared identifier. A firm reporting a failed save gave an operator nothing to search: no trace id, no route label, no latency, and no way to tell whether the service was reached. P1-11 will generate services, so the next one must inherit observability rather than copy it.

**Done**
- [`crates/timelex-telemetry`](../crates/timelex-telemetry) is the workspace's only OpenTelemetry dependency (`opentelemetry` 0.33, `tracing-opentelemetry` 0.34). Three entry points: `init(service)`, `instrument(router, service)` and `inject(&mut HeaderMap)`.
- `instrument` runs after routing, so every request gets one server span named `METHOD /matched/route` (raw path when nothing matched) with method, path, route, status and `ERROR` on 5xx, plus `timelex.http.server.requests` (counter) and `timelex.http.server.duration` (histogram, seconds) labelled by method, route and status. An unmatched path is labelled `unmatched`, so cardinality follows the route table.
- `init` installs one subscriber: the OpenTelemetry layer plus a `fmt` layer whose event format writes JSON or plain text (`TIMELEX_LOG_FORMAT`) prefixed with the service name. Sensitive field names and bearer tokens are replaced with `[redacted]` before formatting. `TelemetryGuard` shuts the providers down on exit, flushing batches.
- Propagation is W3C `traceparent` only. The gateway extracts an incoming trace in the middleware and injects the current context in `authenticate`, `forward` and `relay`; identity and the vault extract from their own inbound request in the same middleware.
- Export is opt-in: with `OTEL_EXPORTER_OTLP_ENDPOINT` set, a batch span exporter and periodic metric reader send OTLP/HTTP protobuf; without it, ids are still real and still propagated.
- Wiring: `app_with` instruments the gateway's public and internal routers, `http::router` instruments identity and vault, and each `serve` calls `init` and keeps the guard. `keygen` and `migrate` do not install telemetry.
- Tests: 9 in the crate (a combined propagation test that reads exported spans with route and status attributes, a metrics test with an in-memory reader, three log-format tests and four redaction tests), plus a gateway test over the stub identity: one caller trace contains the gateway span and the service's child span, both internal calls carry the caller's trace id and the gateway's span id.
- [ADR-0016](adr/0016-telemetry-kit.md); TASKS, INDEX, ARCHITECTURE, the root README layout and the deploy readme are updated; compose sets `TIMELEX_LOG_FORMAT=json`, `RUST_LOG=info` and documents the endpoint variable.
- Verified beyond the tests: the rebuilt image and `docker compose up --wait` brought all five containers up healthy; a tenant was created and a `generic_api_key` written and listed through the gateway (201 with matching last four, list 1), the vault relay emitted `secret.created`, and no value appeared in a log. The gateway binary also served `/health` with a live OTLP endpoint configured and JSON logs, so `init` builds both exporters inside the tokio runtime without blocking.

**Decisions and why**
- **One crate, not per-service setup.** Three copies of the middleware is how span names, the propagation format and the redaction list drift; P1-11 would generate a fourth.
- **Opt-in export, always-on propagation.** A service that cannot reach a collector would otherwise fill its batch queue and retry, and the dev stack would need a collector to stay quiet. One environment variable turns export on without a code change.
- **`i64` for the status attribute.** `record_u64` in `tracing-opentelemetry` falls back to `record_debug`, which exports the status as a string; the test pins the numeric type.
- **Record the status inside the instrumented future.** A span ends when the instrumented future completes, so recording afterwards closes the span without the attribute.
- **Redaction before formatting.** A collector must never receive the value; the filter runs in the event format, which both the JSON and plain paths use.
- **The test seam is `#[doc(hidden)]`.** `install_for_tests` takes a caller-owned tracer, so the gateway asserts real parent-child span links without a collector and the production path never links the testing code.

**Next:** tighten redaction tests with a field named like a body when P2-02 starts logging capture payloads; add MCP request spans and per-model-call records to the same crate as those features land; add a collector service to compose when a deployment first needs one. P1-11 (new-service generator) and P1-13 (MCP host skeleton) are the remaining Phase 1 gaps; P1-12 is branch protection, a shared repository setting.

## 2026-11-01: P1-07 compose stack — five healthy containers and a working secret
**Trigger:** every service so far was started with `cargo run` against a hand-prepared Postgres, and the internal token, master key and operator credentials were pasted into shells. A new contributor could not bring the platform up with one command, and CI never saw the services talk to each other.

**Done**
- [`Dockerfile`](../Dockerfile) builds one image: a `rust:1-bookworm` builder with BuildKit cache mounts for the registry and `target`, the three binaries copied to `/out` (a cache mount does not persist into a layer), and a `debian:bookworm-slim` runtime with `ca-certificates` and `curl`, non-root `USER 10001:10001`, and the entrypoint.
- [`deploy/docker-compose.yml`](../deploy/docker-compose.yml): Postgres 17 on host port 55433, SeaweedFS 4.51 in `mini` mode on 9100, a one-shot `keys` container, one-shot `identity-migrate` and `vault-migrate` containers, and the identity, vault and gateway services with healthchecks and `depends_on` conditions. `docker compose up --wait` returns only when all five are healthy. The gateway publishes `127.0.0.1:8088`.
- [`deploy/container/generate-keys.sh`](../deploy/container/generate-keys.sh) is idempotent: it runs the gateway's own `keygen`, writes the public keys and the operator-key and master-key hashes to `/keys/env`, and stores the operator key in `/keys/operator-key` with mode 0444. [`deploy/container/entrypoint.sh`](../deploy/container/entrypoint.sh) exports that file (`set -a`) and execs `$TIMELEX_BINARY`.
- [`deploy/postgres/dev/005_dev_logins.sql`](../deploy/postgres/dev/005_dev_logins.sql) grants `LOGIN` to the migrator and service roles for the compose stack only; the bootstrap roles stay passwordless and `NOLOGIN`. Compose sets `POSTGRES_HOST_AUTH_METHOD=trust`, publishes Postgres on loopback only, and both files say the stack is not a deployment manifest.
- [`.dockerignore`](../.dockerignore) keeps `target`, `node_modules`, docs and the plugin out of the build context; [`.gitattributes`](../.gitattributes) pins LF for shell scripts and the Dockerfile, because a CRLF entrypoint fails with a cryptic `exec format error`.
- [Deploy readme](../deploy/README.md) is rewritten with the service table, up/ps/curl commands, key retrieval, a tenant-creation example, the usage endpoint, and the security caveats. The root README quick start and the Postgres readme point at it.

**Verified**
- All five containers healthy (`docker compose ps`, `up --wait` exit 0).
- `/health` returns `{"status":"ok"}` and `/v1/meta` returns the gateway's version, contracts and stage.
- A tenant was created with the generated operator key (201), then a `generic_api_key` was written through the gateway with a value generated at run time: 201, metadata only, `lastFour` matching, `allowedHosts: ["example.test"]`; list showed one, `DELETE` returned 204, list showed zero.
- The vault relay logged `secret.created` then `secret.revoked` with metadata only, and neither the vault nor the gateway log contained the value.
- Unauthenticated and invalid-key requests to `/v1/secrets` returned 401.

**Decisions and why**
- **SeaweedFS instead of MinIO.** No MinIO tag resolves from Docker Hub or Quay any more (checked 1 Nov 2026). SeaweedFS's `mini` mode is a single container with master and S3 and an HTTP cluster-status endpoint, which makes the healthcheck a plain `wget`.
- **Non-default host ports.** A developer machine that already runs Postgres and an S3 gateway cannot start the stack otherwise; 55433 and 9100 avoid the collision and are documented. Container-to-container traffic still uses the standard ports.
- **One image, three binaries.** The services share the workspace dependency graph; three images would compile it three times. `TIMELEX_BINARY` picks the process.
- **Keys are generated into a volume, never committed.** The operator key is never printed by `up`; the readme shows how to read it. Re-running the stack keeps the same keys, so tokens and stored secret rows survive a restart.
- **`set -a`, not a plain source.** A sourced assignment stays shell-local; without export the services exited with `TIMELEX_GATEWAY_PUBLIC_KEYS is not set`. The entrypoint exports the whole file once.
- **The healthcheck talks TCP.** `pg_isready` with no host uses the init socket, which answers during the init phase before the server accepts connections; `-h 127.0.0.1` tests what the services will actually use.
- **No worker container yet.** `workers/py-worker` is a library with no entry point; a container that runs nothing would only look healthy.

**Next:** P1-10 telemetry (traces, metrics, redacted logs) is the remaining Phase 1 kit gap and unblocks P1-11's `tools/new-service`. The compose stack is a development convenience, not a production deployment; a real deployment target still needs secrets management and TLS.

## 2026-11-01: P1-09 SDK types generated from the contracts, drift fails CI
**Trigger:** the SDK's `Health`, `Meta` and `ActivityEvent` were typed by hand. A contract change that nobody mirrored would compile and ship, and the first sign of trouble would be a runtime mismatch.

**Done**
- [`tools/sdk-gen`](../tools/sdk-gen) is a private workspace with one script, `generate.mjs`. It renders `packages/sdk/src/generated/gateway.ts`, `identity.ts` and `vault.ts` from `contracts/openapi/*.v1.yaml` with `openapi-typescript` 7.13.0, and `activity-event.ts` from `contracts/events/activity-event.schema.json` with `json-schema-to-typescript` 16.0.0. Every file carries a "do not edit" header naming its source.
- The generator also removes stale generated files (only ones that carry its header) and supports `--check`, which regenerates in memory and exits 1 on a changed, missing or stale file without touching the tree.
- `packages/sdk/src/types.ts` now derives `Health` and `Meta` from the generated gateway paths and re-exports the generated `ActivityEvent`; the hand-written copies are gone. `packages/sdk/src/index.ts` exports `GatewayPaths`, `IdentityPaths`, `VaultPaths` and the matching `operations` and `components` types.
- Root scripts `npm run generate:sdk` and `npm run check:sdk`; the TypeScript CI job runs `check:sdk` between build and tests.
- Verified: `check:sdk` failed before generation (four missing files), passed after; editing `gateway.ts` and adding a stale `old.ts` each made it exit 1; regenerating restored an identical file and removed the stale one. Build (TypeScript 7), the three SDK tests and the link checker pass.

**Decisions and why**
- **The generator gets its own TypeScript.** `openapi-typescript` peer-depends on TypeScript 5 and uses the classic compiler API; TypeScript 7 ships a different API, so installing it into the SDK workspace fails with `ERESOLVE` and forcing the peer would crash at generation time. A private workspace pinned to TypeScript 5.9.3 keeps the SDK on 7 and makes the constraint explicit instead of hiding it in an install flag.
- **The event schema is generated too.** It is JSON Schema rather than OpenAPI, but it is a contract with a hand-mirrored type and a golden example; leaving it manual would recreate the same drift risk for the event that every derived record cites.
- **Drift is checked by regeneration, not by `git diff`.** The check works on a dirty tree, on Windows checkouts with different line endings (comparison normalises CRLF) and in a fresh clone, and it reports exactly which contract file disagrees.
- **Generated files are committed.** Consumers and reviewers can read the types without running Node, and the drift check has a baseline to compare against.

**Next:** generate the secret-event schema when the SDK exposes secret events, and add the generator's output check to `tools/new-service` (P1-11) so a new service starts with a contract-locked client. P1-07 compose and P1-10 telemetry remain the other Phase 1 gaps.

## 2026-11-01: P1-14 audit chains — one verifier for every service
**Trigger:** the vault grew a tamper-evident `secret_audit` chain in P1-05, but the hashing and checking lived inside the vault domain crate. The time ledger (P4-01), autonomy decisions and billing will all owe an audit chain, and three copies of a hash layout is how a chain stops being trustworthy.

**Done**
- [`crates/timelex-audit`](../crates/timelex-audit): `AuditEntry` (seq, microsecond time, kind, actor, subject UUID, non-secret detail), `StoredEntry` for rows read back, `entry_hash(domain, prev, entry)` and `verify(domain, &[StoredEntry]) -> Result<Hash, ChainError>`. `GENESIS` is 32 zero bytes. Every field is length-prefixed, and the service domain is hashed first, so two services' entries are never interchangeable.
- 12 contract tests: deterministic hashing and field coverage, domain separation, byte-shift resistance, a good chain verifying to its head, an empty chain returning `GENESIS`, and detection of an edited entry, a dropped entry, a chain not starting at 1, reordered entries, a forged `prev_hash`, and a tail rehashed around an edit (caught by comparing the head to a separately stored anchor).
- The first test pins the vault v0 hash for a known entry (`769a1a…c1ef`). The byte layout is unchanged, so chains written before the library existed still verify.
- [`crates/timelex-vault/src/audit.rs`](../crates/timelex-vault/src/audit.rs): the vault now builds a library entry from its `AuditRecord` with domain `timelex-vault` and delegates hashing. Its action type and stored schema are untouched.
- [`services/vault/tests/store.rs`](../services/vault/tests/store.rs): the chain assertion now reads the stored rows into `StoredEntry`s, verifies them with `timelex_audit::verify`, and compares the returned head with `vault.chain_heads`. Every existing vault test exercises it, so the real schema and hash format stay covered.

**Decisions and why**
- **One layout, domain-separated.** The domain is part of the hash input, so a `secret.created` entry cannot be lifted into another service's chain even if the other chain has the same shape.
- **Verify returns the head.** Checking links alone cannot catch an attacker who recomputed the whole tail. Comparing the head with an independently stored value (or an external anchor later) can. The library does not choose the anchor; the service does.
- **Microseconds, because that is Postgres's precision.** Hashing a value that storage truncates would make a stored chain fail to verify against its own rows.
- **No schema or action types in the library.** Each service keeps its own vocabulary and table; only the chain rules are shared.
- **The vault migrates now, not later.** It is the only chain with real rows and tests, so leaving it on a private copy would keep two implementations alive and make the compatibility test meaningless.

**Next:** use the same library for the autonomy and ledger chains when those tables land (P4-01), and add an export or anchoring path so the head can be compared outside the database. P1-07 compose and P1-10 telemetry remain the other Phase 1 gaps.

## 2026-11-01: P1-15 MCP adapter — one pinned, safe transport
**Trigger:** research 08 verified `rmcp` 3.6.0 on the server side, but its defaults are built for a stateful intranet server: legacy sessions on, per-request metadata optional, Origin validation off, every protocol version accepted, and a loopback-only Host list. Any service wiring `rmcp` directly would have to remember all five settings.

**Done**
- [`crates/timelex-mcp`](../crates/timelex-mcp) is now the workspace's only `rmcp` dependency, pinned to `=3.6.0`. `Config::new` requires at least one full origin (`scheme://host[:port]`) and one allowed `Host`, and `with_max_request_body_bytes` refuses zero (default 1 MiB); failures are typed `ConfigError`s. `Config::transport` sets `legacy_session_mode = false`, `stateless_protocol_metadata_required = true`, the explicit allow-lists and the limit. `router` mounts the handler at `/mcp` with `NeverSessionManager`.
- `SUPPORTED_VERSIONS` and `supported_versions()` advertise only 2026-07-28. The adapter's test handler overrides `ServerHandler::supported_protocol_versions` with it, so discovery lists one version and a legacy `initialize` gets `-32022` and no negotiated version.
- [`tests/probes.rs`](../crates/timelex-mcp/tests/probes.rs): 23 contract tests. Each probe starts a real axum server on an ephemeral loopback port and repeats the 18 checks in `spike/probe.sh` — discovery, `tools/list`, `tools/call`, Tasks create and cross-request `tasks/get`, missing `_meta`, header/body mismatches, unsupported and conflicting versions, GET/DELETE 405, ignored `Mcp-Session-Id`, bad Origin and Host, legacy `initialize` — plus a strict-metadata case and five configuration checks. All pass.
- [ADR-0015](adr/0015-mcp-adapter.md) records the decision; TASKS, INDEX, ARCHITECTURE and the README layout are updated.

**Decisions and why**
- **Defaults are replaced in code, not in deployment policy.** Origin validation is a spec MUST, and a deployment that forgets the list should fail to start, not serve unvalidated traffic.
- **Only 2026-07-28.** No TimeLex client speaks an older revision and the gateway already requires that header ([ADR-0013](adr/0013-gateway-limits-and-metering.md)); accepting the rest only widens the surface.
- **The contract tests pin behaviour, not just our wrapper.** Probe 18 showed `rmcp` delivers the legacy rejection over SSE with HTTP 200 and `-32022`; the test asserts the rejection and that no version was agreed, so an SDK change that starts negotiating would fail CI without pinning an incidental status.
- **The adapter stops at the transport.** Task state stays in memory in the test only; a real service must put task rows in Postgres before advertising the Tasks extension.

**Next:** wire the first service's MCP tools through the adapter (vault tools need a durable task store for anything long-running), and leave `rmcp` bumps to a reviewed PR that runs these probes. Gateway aggregation (`tools/list`, SSE, task-id routing) is still open. P1-07 compose and P1-10 telemetry remain the other Phase 1 gaps.

## 2026-10-31: P1-06 outbox and relay — the vault publishes `secret.*`
**Trigger:** the vault could change a key with no way for another service to learn about it. P2-02 (capture-hub ingest) and the audit consumers need the event path first.

**Done**
- [`contracts/events/secret-event.schema.json`](../contracts/events/secret-event.schema.json) and a golden [`secret-event.json`](../contracts/examples/secret-event.json): envelope (`schemaVersion`, `ev_` id, `type`, `source: vault`, `tenantId`, `time`, `actor`, `data`), metadata only, and `result` required exactly for `secret.tested`. [`timelex-types`](../crates/timelex-types/src/lib.rs) mirrors it with `deny_unknown_fields`; 7 tests validate the example against the schema and round-trip every variant.
- [`crates/timelex-outbox`](../crates/timelex-outbox): `EventId`, `Event`, `append` on the **writer's** connection, `Publisher` with a JSON-line stdout sink, and `Relay` (claim with `FOR UPDATE SKIP LOCKED`, 60-second lease, publish before mark, exponential backoff capped at 300 seconds). 2 unit tests and 4 database tests: a rollback leaves no event, a batch publishes once in order, a failed publish backs off and a later poll retries, and a redelivery after a crash has one effect for an idempotent consumer.
- [`deploy/postgres/bootstrap/003_outbox.sql`](../deploy/postgres/bootstrap/003_outbox.sql): `platform.outbox` with a partial pending index. `provision_service` now grants each service `SELECT`/`INSERT`, `UPDATE` on the four delivery columns only, and `USAGE` on the sequence. The provisioning file moved to `004`, so bootstrap runs `001…004`.
- [`services/vault/src/store.rs`](../services/vault/src/store.rs): every write appends its event in the same tenant transaction as the row change and the audit entry — created, updated, tested, rotated, revoked. Revoke and record-test read back the metadata in the same statement; a second revoke writes nothing. The value never enters the event. 12 database tests, one of them covering the whole sequence and scanning every payload for the values.
- [`services/vault/src/main.rs`](../services/vault/src/main.rs): the service starts the relay by default on its own two-connection pool; `TIMELEX_OUTBOX_RELAY=0` disables it and `TIMELEX_OUTBOX_INTERVAL_MS` sets the poll interval.
- [`services/gateway/tests/e2e.rs`](../services/gateway/tests/e2e.rs): the provider-key journey now asserts the five events in order, their tenant, their secret id, that the type column matches the envelope, and that no value appears in any payload.

**Decisions and why**
- **One `platform.outbox` for all services** ([ADR-0014](adr/0014-transactional-outbox.md)). The relay must read across tenants, and every service table is forced under row-level security with no bypass. The `platform` schema already sits outside the "every table has a tenant policy" guard, and column-limited grants keep producers from rewriting published rows.
- **Publish first, mark second.** A crash between the two redelivers; it can never lose the event. The claim lease makes a stuck relay's batch available again.
- **At least once in the relay, exactly once in the effect.** The envelope id is the dedupe key; a test proves a duplicate delivery changes nothing.
- **The queue order is a claim order, not a promise across services.** `seq` is insert order, so consumers rely on facts and timestamps rather than a global sequence.
- **Bootstrap order matters.** The table exists before provisioning runs, so the grant lives inside `provision_service` and future services inherit it.
- **No `LISTEN/NOTIFY` yet.** Polling is the durable path; a notification would only shorten latency.

**Next:** P1-07 compose stack and P1-10 telemetry, or P2-02's idempotent ingest as the first real consumer; the vault still waits on P1-13 for leases and P1-15 for MCP tools.

## 2026-10-31: gateway routes the vault — contract 0.4.0
**Trigger:** P1-05 increment 3 merged as `09f33bb` (#21), and the dependabot queue was cleared; the vault had no caller yet.

**Done**
- [`contracts/openapi/gateway.v1.yaml`](../contracts/openapi/gateway.v1.yaml) 0.4.0: `GET /v1/secrets` (optional `includeRevoked`), `PUT /v1/secrets/{kind}`, `DELETE /v1/secrets/{id}`, `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, each requiring a tenant API key and referring to the vault contract's schemas. 409 and 422 pass through; 401 and 5xx are still hidden by the gateway.
- [`services/gateway/src/proxy.rs`](../services/gateway/src/proxy.rs): a `Target` describes each REST route (label, service, method, canonical path, the 400 detail when it has no path). `internal_token` now takes the audience, and `forward` takes the service and method, so the same `handle` serves identity and the vault. Tokens for secrets are minted for `vault`.
- Validation before forwarding: the four kinds against a fixed list, ids by parsing and re-formatting as UUIDs, and the list query accepted only as absent, `includeRevoked=true` or `includeRevoked=false`. A bad parameter is a 400 *after* authentication and rate limiting, and nothing reaches the vault.
- [`services/gateway/tests/routing.rs`](../services/gateway/tests/routing.rs): 3 stub tests (forwarding, audience and body; canonical paths, query and 400s; an unconfigured vault is 502). [`services/gateway/tests/e2e.rs`](../services/gateway/tests/e2e.rs): a real vault service with its own schema, role and migrations in the same throwaway database, 3 tests (store/list/test/rotate/revoke with a `settings:manage` refusal, cross-firm invisibility, and the vault refusing tenant keys, forged tokens and operator tokens). 27 routing and 9 end-to-end tests pass.

**Decisions and why**
- **PUT and DELETE share `/v1/secrets/{key}`**, as the vault's router does: axum will not register one segment under two parameter names, and the wire paths stay exactly the vault contract's.
- **The gateway validates kinds and ids itself.** It has no business rules otherwise, but these two decide a URL, and forwarding a client's raw text would let `..%2f` tricks and unknown kinds reach the vault. Canonical re-formatting keeps every forwarded URL one the contract describes.
- **One query parameter, positively allowed.** `includeRevoked` is the only query the route has; anything else is refused rather than dropped, so a caller cannot believe a filter was applied when it was not.
- **A vault with no configured URL is 502**, not 501: the route exists, the service behind it is unreachable, and the contract already says 502 for that.
- **The permission stays the vault's.** The gateway forwards the principal's permissions and the vault enforces `settings:manage` in the store, so a new route cannot forget the check.
- **Test values are built at run time** (`sk-ant-api03-<uuid>`), and the end-to-end test builds the vault master key from two UUIDs, so no credential-shaped literal enters the repository.

**Next:** P1-06 outbox and relay, starting with `secret.*` events; then P1-15 MCP tools for the vault and P1-13 `mcp-host`.

## 2026-10-30 (latest): P1-05 vault v0 finished — HTTP API and the Anthropic check
**Trigger:** increment 2 merged as `8ef8214` (#20); the last part is the service the gateway and the admin UI will talk to.

**Done (increment 3)**
- [`services/vault/src/http.rs`](../services/vault/src/http.rs): all five contract routes plus `/health`. Internal tokens of kind `principal` only; the firm, user and permissions come from the verified token. Problem details (RFC 9457) for every failure, a 64 KiB body limit, and metadata JSON that omits `lastTestedAt`, `lastUsedAt` and `revokedAt` until they exist instead of sending null.
- [`services/vault/src/validator.rs`](../services/vault/src/validator.rs): a `ProviderValidator` trait, so the HTTP layer is tested with a script and no network. The live validator makes one read-only `GET https://api.anthropic.com/v1/models?limit=1` for `anthropic_api_key` with the key in `x-api-key`, and reads only the status: 2xx accepted, 401/403 rejected, anything else (429, 5xx, timeout, refused connection) unreachable. Other kinds pass on shape, as 0.1.0's contract says.
- [`services/vault/src/main.rs`](../services/vault/src/main.rs): `serve` and `migrate`; the master key is base64 of exactly 32 bytes, decoded into a zeroising wrapper, and no error message prints it.
- 5 unit tests (status mapping, header placement against a local stand-in server, timeout/refusal) and 13 HTTP tests against Postgres: create/list/revoke, replace under the same id, rotation, five credential failures mapping to one 401, wrong kind and missing permission mapping to 403, malformed and 70 KiB bodies (400/413), unknown kind and a host outside the kind (400), a passing test stamping only `lastTestedAt`, a rejection as 422, an outage as 502 with no stamp, and another firm's id as 404 for every operation.
- Every response in those tests is searched for the full value; it never appears. `Debug` for `SecretValue`, `LoadedSecret` and `PutSecretRequest` is redacted, which the tests assert, and the only log line the service writes is a fixed sentence plus a database or rule message that never holds a value.

**Decisions and why**
- **Axum cannot register `{kind}` and `{id}` on the same segment**, so one route owns `/v1/secrets/{key}`: PUT reads it as a kind, DELETE as a UUID. The wire paths are the ones the contract describes.
- **An unreachable provider is not recorded as a test.** A 502 means no verdict; writing `lastTestedAt` would suggest the key failed and could push an admin to revoke a working key. A rejection is a verdict and is recorded.
- **The provider's own words are never relayed**, and a rejection body is fixed text. A provider error can contain the key or account details.
- **`includeRevoked` is parsed by our own extractor**, so a bad query is a problem-details 400 instead of axum's plain-text rejection.
- **Test keys are drawn at run time** (uuid 4), after CodeQL flagged fixed key and nonce literals in increment 2 as hard-coded cryptographic values.

**Still to come:** a gateway route for `/v1/secrets` (the next password-like settings also need `settings:manage`); `secret.*` events through the outbox (P1-06); the lease (P1-13); MCP tools through the P1-15 adapter; KMS/WIF (T0-L05).

## 2026-10-30 (later): P1-05 vault v0 — rules, sealed schema and store
**Trigger:** increment 1 (contract and docs) merged as `9a7c066`; continuing with the part that has to be right before any endpoint exists.

**Done (increment 2)**
- [`crates/timelex-vault`](../crates/timelex-vault), a no-I/O crate: the four kinds and their provider hosts; validation of value (printable ASCII, 8–4096), label and hosts (narrowing only, never widening); a zeroising `SecretValue` with redacted `Debug`; envelope encryption (AES-256-GCM per secret, data key wrapped behind a `KeyWrapper` trait) with tenant, secret id and kind bound into **both** seals; and the per-firm audit hash chain. Entropy and the clock are parameters, so every rule is tested with fixed inputs (27 unit tests, 1 doctest).
- [`services/vault/migrations/0001_vault.sql`](../services/vault/migrations/0001_vault.sql): `secrets` (sealed columns NULL together, coarse CHECKs as a second line, a partial unique index giving one live secret per kind, a CHECK that a revoked row cannot hold a value), `secret_audit` (append-only; no UPDATE or DELETE grant) and `chain_heads`. All three get the standard tenant policy; `vault` is provisioned in the bootstrap.
- Store (`services/vault/src/store.rs`): put (create or replace), list metadata, rotate, revoke, open-for-test, record-test. Each runs in one tenant transaction and appends one audit entry under a `FOR UPDATE` lock on the chain head, so chains cannot fork. Revocation NULLs every sealed column.
- 11 database tests: value never stored, replace destroys the old value, rotate clears the test stamp, revoke keeps metadata, another firm sees nothing, no permission writes nothing, the database refuses a clear value / bad host / rewritten audit / deleted secret, every vault table has the policy, two firms have chains that both start at 1.

**Decisions and why**
- **Binding in the seals, not just the row.** Copying a row to another firm, or relabelling a key's kind, makes it fail to open rather than be used against the wrong provider.
- **Replacement, rotation and revocation all destroy the old data key**, so a database copy taken later cannot decrypt a superseded value. Revoked rows keep metadata (provider, last four, when) and nothing else.
- **One live secret per kind is a database guarantee**, not a code convention: a partial unique index. A PUT racing another PUT retries once and replaces.
- **The audit chain is per firm and written in the same transaction as the change.** A separate audit writer would allow a change with no entry.
- **Testing is not use:** `lastTestedAt` is set by `record_test`; `lastUsedAt` is reserved for leases and stays NULL in v0.

**Next:** increment 3 — the HTTP layer (internal `principal` tokens, `settings:manage`, problem details, 64 KiB limit), the validator trait with one real Anthropic check, the binary, and HTTP tests proving no plaintext is returned or logged.

## 2026-10-30 (even later): P1-05 vault v0 starts with the contract
**Trigger:** After gateway part b merged (#18), Kone chose P1-05 (vault v0) next, with REST only in this version (MCP waits for P1-15) and a validator trait plus one real Anthropic check.

**Done (increment 1)**
- [`contracts/openapi/vault.v1.yaml`](../contracts/openapi/vault.v1.yaml) 0.1.0: `PUT /v1/secrets/{kind}` (create or replace), `GET /v1/secrets` (metadata only), `POST /v1/secrets/{id}/test`, `POST /v1/secrets/{id}/rotate`, `DELETE /v1/secrets/{id}` (revoke, idempotent). Typed kinds (`anthropic_api_key`, `m365_oauth`, `google_oauth`, `generic_api_key`), allowed hosts that may narrow but never widen a kind's defaults, and metadata that never contains a value.
- Callers are internal tokens of kind `principal` holding `settings:manage`; the firm comes from the token (ADR-0012). Only the gateway reaches the service.
- Vault README, architecture table and task list updated. OpenAPI validated with `openapi-spec-validator` (all three contracts pass).

**Decisions and why**
- **REST only in v0.** The lease (`/internal/v1/secrets/{id}/lease`) has no consumer until `mcp-host` (P1-13) and is the most dangerous surface, so it lands with its first caller and its own tests. The MCP tools are served through the P1-15 adapter, not hand-rolled twice.
- **`settings:manage` rather than a new permission.** Secrets are firm settings and the permission is human-only; adding `secrets:*` would mean touching identity's permission model and contract for no v0 gain. A dedicated permission can come with the lease, when non-human callers need scoped access.
- **Write-only by shape.** There is no GET by id and no endpoint that returns a value; a caller gets the last four characters so two keys can be told apart.
- **Revoke destroys the wrapped data key**, not just a flag, so a later database copy cannot decrypt the value.
- **Test is not use.** `lastTestedAt` and `lastUsedAt` are separate; only an internal caller using the secret sets the latter.
- **KEK from the environment behind a `KeyWrapper` trait** for 0.1.0 (KMS and WIF wait on T0-L05), so the storage format does not change when a KMS adapter arrives.

**Next:** increment 2 — `crates/timelex-vault` (kinds, validation, zeroising value, envelope encryption) plus the `vault` schema, bootstrap role and store with database tests.

## 2026-10-30 (later still): P1-04 part b — MCP header routing
**Trigger:** PR B merged (#17); finishing part b with `POST /mcp`.

**Done (PR C)**
- `services/gateway/src/mcp.rs`: compiles `contracts/mcp/routes.json` into the build (`include_str!`), refusing duplicates, empty names and an empty table at start-up. `classify` decides from the three headers alone: version must be `2026-07-28`, `Mcp-Method` is validated, and only `tools/call` needs `Mcp-Name`. A valid method it does not route is `Unrouted`, not malformed.
- `services/gateway/src/proxy.rs`: `POST /mcp` authenticates the tenant key, spends the firm's budget, classifies the headers, looks the tool up, mints a token addressed to the *owning* service and forwards the body untouched with an allowlist of headers (`Content-Type`, the three MCP headers, `Origin`). Unknown tool 404; unrouted method or unconfigured service 501; malformed headers 400 after authentication.
- `Trip` now carries route, service and tool; the meter record gains optional `service` and `tool`.
- 8 new routing tests with a second stub service: routing and audience, the header allowlist, an identity-owned tool, unknown tool, 501s, malformed headers (and 401 before any 400), operator keys refused, and the firm budget covering MCP.
- README, TASKS and the worklog updated. Contract unchanged (already 0.3.0 from PR A).

**Decisions and why**
- Routing happens after authentication and the budget check, like the REST routes, so an unauthenticated caller cannot probe the table and a firm at its limit cannot make the gateway read a body.
- `Origin` travels on because the service must enforce it (MCP 2026-07-28); nothing else the client sent does.
- The token's audience is the owning service, not identity, so a tool call cannot be replayed against identity.
- The table is compiled in, so a deployment carries no file that can drift from the code.

**Verified:** `cargo fmt --all`, `cargo clippy -p timelex-gateway --all-targets -- -D warnings`, `TIMELEX_SKIP_DB_TESTS=1 cargo test -p timelex-gateway` (25 unit + 2 contract + 24 routing + 6 end-to-end passing; the end-to-end tests pass early locally and run against Postgres in CI).

**Next:** P1-05 (vault v0), P1-06 (outbox) or P1-15 (`timelex-mcp` adapter and MCP aggregation).

## 2026-10-30 (later): P1-04 part b — rate limits and metering
**Trigger:** Continuing part b after the contracts PR merged (#16).

**Done (PR B)**
- `services/gateway/src/limits.rs`: token buckets keyed per firm and per operator; fake-clock tests for burst, refill, isolation and a clock going backwards. The map recovers from a poisoned mutex rather than taking the gateway down.
- `services/gateway/src/meter.rs`: one metadata-only JSON record per handled request to stdout; running totals per firm; `GET /internal/usage` payload with `since` and uptime.
- `services/gateway/src/config.rs`: `TIMELEX_SERVICE_URLS` (validated, `identity` required) replaces `TIMELEX_IDENTITY_URL`; `TIMELEX_RATE_LIMIT_PER_MINUTE`/`_BURST` (default 600/120); `TIMELEX_GATEWAY_INTERNAL_ADDR` (default `127.0.0.1:8089`). Bad values fail at start-up; zero is refused so a typo cannot lock a firm out.
- `services/gateway/src/proxy.rs`: authentication, then the budget check, then the body read and forward. One `Trip` records every exit path: 401, 429, 400, 413, 502/504 and success. 429 carries `Retry-After`.
- `app_with` now returns `Gateway { public, internal }` so the binary binds the usage listener separately and warns if it is not loopback. Tests updated; 3 new routing tests and 1 new end-to-end test cover 429, operator isolation, internal-only usage and per-firm totals.
- READMEs, architecture table and task list updated.

**Decisions and why**
- The budget is spent **after** authentication, so a 429 still costs one `authenticate` call. That is the price of not trusting the key prefix; documented as the known amplification gap rather than solved twice.
- The 429 request's body is never read and nothing is forwarded, so a firm at its limit cannot make identity or the service do work.
- Usage is served from a second listener, not the public one; a routing test asserts `/internal/usage` is a 404 on the public app and `/v1/api-keys` is a 404 on the internal one.

**Verified:** `cargo fmt --all`, `cargo clippy -p timelex-gateway --all-targets -- -D warnings`, `TIMELEX_SKIP_DB_TESTS=1 cargo test -p timelex-gateway` (20 unit + 2 contract + 16 routing passing; 6 end-to-end pass early without a database, CI runs them on Postgres).

**Next:** PR C (`POST /mcp` routed by `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` through `contracts/mcp/routes.json`, 501 for unrouted methods).

## 2026-10-30: P1-04 part b starts with contracts
**Trigger:** Kone asked to pick the system up again and chose gateway part b (rate limits, metering, `Mcp-Method` routing). Context reload confirmed the ClearCut boundary: billing calls ClearCut directly (ADR-0008); ClearCut is not a gateway target.

**Done (PR A, contracts only)**
- `contracts/openapi/gateway.v1.yaml` 0.3.0: 429 with `Retry-After` on every authenticated operation, a `POST /mcp` operation routed by `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name`, and 501 for methods not routed yet.
- `contracts/mcp/routes.json`: tool-to-service routing for all 26 tools in the catalogue, with read-only flags.
- `services/gateway/tests/contracts.rs`: parses `tools.md` and fails if the catalogue and `routes.json` drift (2 tests pass).
- [ADR-0013](adr/0013-gateway-limits-and-metering.md) accepted: per-firm token buckets after authentication, metadata-only metering to stdout plus a loopback `/internal/usage`, header routing without body parsing, and the config changes.
- Docs index, contracts README and MCP catalogue updated.

**Decisions and why**
- Limits are keyed **after** authentication. The tenant id in the key prefix is unverified; a pre-auth bucket could be spoofed to burn another firm's budget.
- The limiter is in process, so the effective limit multiplies with replicas until a shared store exists. Phase 1 runs one gateway.
- Metering stays out of Postgres: the gateway owns no data, and P1-06/P1-10 become the durable path. This is request metering, not model spend (P3-12).
- `tasks/*` cannot be routed by `Mcp-Name` because it carries the task id; P1-15 must prefix task ids with a service or add a lookup.

**Known gaps**
- Invalid keys still reach identity on every attempt; identity-side or edge per-IP limits are the later fix.
- SSE/streaming, OAuth, tool schemas and MCP aggregation wait for P1-15.
- Usage counters reset on restart; `since` and uptime are in the response.

**Next:** PR B (rate limits and metering), then PR C (MCP routing).

## 2026-10-21: Merged the open PRs
**Trigger:** Kone asked to merge every open PR and close any that should not be merged.
- **Merged, in order, with merge commits** (so each stacked PR still applied cleanly after its base landed): #4 identity domain crate, #5 database layer and schema, #6 identity store and contract, #7 P1-04 contracts and ADR-0012, #8 P1-04 part a (internal token, identity HTTP, gateway authentication and routing); then Dependabot #3 (GitHub Actions bumps, pinned by SHA).
- **Closed, not merged:** #1 `postgres:17` to `postgres:18`. Tested: the container exits because the Compose volume is mounted at `/var/lib/postgresql/data`, a path the 18 image no longer supports. Moving to 18 means changing the mount and re-running the database tests and CI on 18; it needs its own PR.
- **#2** (`jsonschema` 0.30 to 0.59) conflicted with `Cargo.lock` after the merges; Dependabot was asked to rebase it. Outcome below if resolved this session.
- GitGuardian flagged synthetic test credentials on #8 (Basic auth header, key-shaped Bearer literal). Fixed by building them at run time; the rule is in CLAUDE.md now.
- CLAUDE.md status sections refreshed (they still described the skeleton).

## 2026-10-20 (session 3): Step 3 begins: P1-01 to P1-03 (schema, tenancy, identity domain)
**Trigger:** Kone approved the Step 1 report and the drafted schema (tenant-in-key tokens, no passwords, identity-only provisioning).

**Done**
- Pushed the Step 1 commits to `main`. From here, work goes through small PRs.
- PR 1: `timelex-identity` domain crate (pure; 55 unit tests + doctest). PR 2 (stacked on PR 1): bootstrap SQL, identity migration, `timelex-db` crate (`TenantPool`/`TenantTx`), 18 database tests, CI Postgres service, ADR-0011.
- Applied the SQL for the first time to a throwaway Postgres 17 (trust auth, no passwords); all 18 tests passed first run, then a **mutation check** (policy replaced by `USING (true)`) made 7 of 11 tenancy tests fail, so the tests do bite.

**Later in the session (PR 3, stacked on PR 5):** `contracts/openapi/identity.v1.yaml` (validated with `openapi-spec-validator`), two domain helpers (`require_permission`, `effective_permissions`), the DB test harness moved into `timelex_db::testing` (feature `testing`), and the identity store crate `timelex-identity-service`: create firm, issue key, revoke key, authenticate, with 16 database tests. Mutation checks: skipping the usable-key check broke 4 tests; dropping role narrowing broke 1.
- **Design finding:** a key's permissions are its scopes intersected with the owner's *current* roles, so demoting someone shrinks their existing keys at once.
- **Open (decide later):** `keys:manage` lets a holder issue a key up to any user's permissions (documented; only admins have it). Idempotency keys (ADR-0010) are accepted but ignored: replaying key issuance cannot return the token twice, so it needs a design decision.

**PR 4 (stacked on PR 3): P1-04 part a.** Contracts first (ADR-0012 accepted; `internal-auth.md`; identity and gateway OpenAPI 0.2.0), then: `timelex-auth` (EdDSA internal token, 20 tests including alg-none, HMAC-with-public-key, forged-key, wrong-audience and tampered-claim attacks); identity HTTP layer and binary (11 tests); gateway authentication and routing (13 stub-upstream tests, 5 end-to-end tests with Postgres); a manual smoke test of the real binaries with `curl`.
- Mutation checks (by hand): operator check bypassed, identity ignoring caller kind, and a leaked extra header each broke at least one test. The leak mutation was *not* caught at first (the test blocked named headers); it now uses an exact header allowlist.
- **Finding:** reqwest cannot tell a connect timeout from a response timeout, and on Windows a closed port times out instead of refusing, so "service down" can read as 504 there. The contract wording still holds ("outcome unknown"); the test uses a dropped connection instead of a closed port.
- **Duplicated on purpose for now:** the problem-details helper exists in both the gateway and identity. A shared HTTP kit crate is worth extracting when a third service needs it.
- **Open:** the gateway has no tracing or request ids (P1-10); identity logs refusal reasons to stderr only; operator credential is a single static key hash (no operator accounts); replay protection is the 30-second lifetime only.

**Not done:** gateway part b (rate limits, metering, `Mcp-Method` routing), users/roles/SSO, audit events.
**Unverified:** behaviour on Postgres 18 (Dependabot PR open); hosted Postgres with a restricted admin role (the bootstrap needs `CREATEROLE`); performance of the policy on large tables.

## 2026-10-20 (session 2): Step 1 research pass, no product code
**Trigger:** research in `docs/research/` was partly unverified; Kone asked for the gaps to be closed from primary sources before building (Step 1), with a stop for review before P1-01 to P1-03.

**Done**
- **T0-L03 / T0-L04 (research 03):** read Part XII (rules 54.1, 54.6–54.15, 54.19, 54.20, 54.24, 54.34, 57) in the Acts Online republication of Notice 401 of 2018, the LPC advisory of 10 May 2024 with the court order, POPIA ss71, 72, 21, 22, and the Information Regulator's guidance-notes page. Listed ten software duties. Corrected: Part XII scope (54.1), retention clock (from last entry), audit-report citation (54.24 not 57.2), client-account content (agreed fee flag), payment methods (54.14.15); **s71 is narrower than we wrote**; added 54.9.2.1 (hosted records). Regulator's transfer guidance: not published, announced as planned.
- **T0-L02 (new research 07):** public Anthropic terms, DPA, retention, ZDR, residency, usage policy, key types, workspaces, Workload Identity Federation. Drafted nine questions for Anthropic.
- **P1-08 (new research 08, spike in `docs/research/spikes/p1-08-rmcp/`):** `rmcp` 3.6.0 serves 2026-07-28 statelessly with Tasks; 18 raw probes; defaults need configuration (Origin, session mode, required metadata).
- **GhostPractice (research 01 §8):** 26-row claim register from the vendor's own pages and the acquirer's press release.
- **Kits:** `docs/validation/interview-kit.md` and `demo-checklist.md` for T0-R01 and T0-B01.
- ADR amendments **proposed** in 0003, 0004, 0005, 0007, 0009 (Decision text untouched). Threat model T19–T24 added. TASKS updated (P1-08 done server-side; new P1-15, T0-L05, T0-L06, T0-A03).

**Findings that change plans**
- **Anthropic has no African inference region** (`global` or `us` only); Fable-class models need 30-day retention and **cannot use ZDR**, which breaks the top rung of the escalation ladder for ZDR tenants.
- **Workload Identity Federation** could let TimeLex hold no Anthropic secret; proposed as a second mode, waiting for Anthropic's written answer.
- The usage policy treats **evaluating a worker's performance** as high-risk, which constrains per-fee-earner analytics.
- GhostPractice is **not "Windows on-premise only"**: hosting is the firm's choice, the database is Microsoft SQL, remote-desktop and virtual-workstation access is listed, and **import/export is advertised**. The vendor is silent on an API. The owner opened a strategic review in July 2025 (unverified for 2026).
- ClearCut's note that `rmcp` 3.5.1 spoke 2025-06-18 does not match the crate changelog or the spike (3.5.1 not tested).

**Still unverified / open**
- Part XII from the Gazette itself (we read a republication through a summarising tool; rules 54.2–54.5, 54.16–54.18, 54.21–54.23, 54.25–54.33, 55, 56 unread). POPIA text is from a commentary site, not the Gazette.
- Anthropic's position on key custody and a vendor-run federation issuer; DPA coverage of POPIA; clause numbers as reported by our fetch tool.
- Capterra facts (price, Windows) could not be re-checked (HTTP 403). GhostPractice export formats, licence terms, API.
- `rmcp` client side, `tasks/update`, `tasks/cancel`, auth, load; the `Mcp-Name` rule for task methods.
- Whether captured time or fee records count as "accounting records" under rule 54.9 (counsel).
- Everything in the interview kit's hypotheses H1–H8: no interview has happened.

**Plan impact:** none to phase order or gates. Phase 1 gets one extra task (P1-15) and Phase 1's MCP risk is lower; vault scope in P1-05 stays "stored key only" until Anthropic answers. Step 3 is not started.

**Next:** Kone reviews Step 2 report; then P1-01 to P1-03 (schema shown as SQL and explained before any migration).

## 2026-10-20: Restart with research; scaffold and publish
**Trigger:** Kone judged both earlier TimeLex prototypes too weak to replace GhostPractice and asked for a restart backed by research: multi-language, microservice, MCP-host backend, bring-your-own keys, connected to ClearCut, fully autonomous, public and credible.

**Done**
- Reviewed the two prototypes (static HTML v1; Next.js v2 with a simulated sync gateway) and ClearCut (Rust core, ADRs, research, phases). Adopted ClearCut's conventions: ADRs, research files, PHASES/TASKS, Apache-2.0, "Written by Kone Tshivhinda & Claude", Rand-first costs.
- Researched GhostPractice, the AI time-capture competitors, South African constraints (LPC Accounting Rules Part XII, POPIA s72, Anthropic usage policy), Microsoft Graph capture limits and MCP 2026-07-28. Files `docs/research/01–06`.
- Wrote ADR-0001…0010, vision, architecture, autonomy ladder, threat model, glossary, phases and a 100+ task backlog.
- Built a walking skeleton in three languages: `timelex-units` (14 tests + doctest), `timelex-types` (7 tests, schema-validated), `timelex-gateway` (3 tests), `py-worker` redaction (19 tests), TypeScript SDK stub (3 tests).
- Specified twelve services; contracts v0; plugin placeholder; CI, CodeQL, dependency review, Scorecard, Dependabot, labels, issue/PR templates, governance, security policy.

**Pushback recorded (see research 06)**
1. "Replace GhostPractice" is three products (time/billing, practice core, regulated money). Start with the first; trust accounting is last and gated.
2. "Fully autonomous" is implemented as exception-based, measured autonomy. Fee notes, trust money and legal documents stay human-gated.
3. Microservices need guardrails for one part-time developer: contract-first, service budget, platform kit; merge services behind the same contracts if the kit does not pay off by end of Phase 2.
4. Language choice is by fit (correctness, ecosystem), not raw speed; three-language ceiling.

**Findings that change plans**
- GhostPractice lists **no API** and is Windows on-premise (third-party listings): interop must be through sanctioned exports/imports (ADR-0009).
- Every AI time-capture product found still requires human review of drafts: the autonomy gap is real and also a risk.
- Outlook Graph subscriptions expire in under 7 days (under 1 day for rich notifications), so renewal and gap detection are first-class.
- Illustrative model spend (R13–R56 per fee earner per month) is far below competitor licences (R700–R3 000+), which supports BYO keys; unmeasured.
- Anthropic usage policy requires qualified-person review for legal advice and documents prepared for signing/filing.

**Unverified / open**
- All GhostPractice facts come from third-party listings and a marketing page (no demo seen).
- LPC rules were read as machine summaries; verify against the primary text (T0-L03).
- Information Regulator final transborder guidance not found (T0-L04).
- Rust MCP SDK maturity versus the final spec (P1-08).
- Anthropic retention/ZDR and key-custody terms (T0-L02).

**Next:** see "Immediate next five" in `docs/TASKS.md`.

## 2026-10-20 (later): publication status

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

- Public repo `konethegreat/timelex` is live (Apache-2.0). CI, CodeQL, Sync labels and OpenSSF Scorecard all passed on the first commit.
- Enabled: Discussions, private vulnerability reporting, delete-branch-on-merge. Already on: secret scanning and push protection.
- **Still manual (browser automation was unreliable):** repository topics; dependency graph, Dependabot alerts and security updates (the "Dependency review" workflow fails on Dependabot PRs until the dependency graph is on); a light ruleset; the `v0.1.0` release (tag pushes were refused from the build sandbox, so create the tag in the release form).
- Dependabot has opened three version-update PRs (actions group, `jsonschema` 0.30 → 0.59, Postgres 17 → 18). CI passes on the first two; review each, and do not merge the Postgres 18 bump without checking the volume-layout change.
