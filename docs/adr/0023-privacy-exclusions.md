# ADR-0023: Privacy exclusions are enforced before an event exists

- **Status:** Accepted
- **Date:** 2026-11-02
- **Task:** P2-07 (privacy exclusions and the metadata-only gate)
- **Relates to:** ADR-0006 (immutable activity events), ADR-0011 (tenancy), ADR-0014 (transactional outbox), ADR-0021 (idempotent capture), ADR-0022 (manual quick capture)

## Context

Capture is only trustworthy if a firm can say what must never be captured. A practice may act against an
adverse party, hold a matter a client asked to keep off the system, or simply not want mail from a
personal provider turned into billable time. The product promise is that such a signal does not become
an activity event at all.

Where the filter runs is the whole decision. An activity event is immutable (ADR-0006), is copied into
the outbox (`activity.recorded`), is read by attribution and will be cited by time entries and bills. A
filter after storage therefore cannot undo capture: it can only hide a row that already exists, already
appears in a feed and may already have been read by another service. Deleting the row is worse: it edits
history and is refused by the write-once grant. The only safe point is inside the write transaction,
before the insert.

Two further hazards shape the design. First, the gateway is not the only door: connectors call the store
directly with no user token, so a gateway-side filter would be bypassed by the very sources that capture
most mail. Second, the metadata-only default needs enforcement at storage, not just good intentions in a
connector. A sender that sets `contentCaptured: true` without a redaction pass must be refused before an
unredacted reference enters immutable history; the redaction worker (P0-08) is a tested library, but its
queue and container are not deployed yet.

## Decision

### 1. Three exclusion kinds, one enforced today

A firm configures exclusions of kind `domain`, `client` or `matter`. Domain is enforced at capture now.
Client and matter are stored, listed and removable now, but take effect when attribution exists (Phase 3),
because a raw mail or calendar signal names neither a client nor a matter. Recording them early lets a
firm state its policy once and lets the attribution stage consult it without a schema change. A kind the
contract does not know is refused, so a typo cannot become a silent no-op exclusion.

### 2. Domains are canonicalised, and a match covers subdomains

A domain value is trimmed, loses one leading `@` and one trailing dot, is lower-cased, and must be an
ASCII domain of at most 253 characters with labels of 1 to 63 letters, digits or inner hyphens. The
stored form is the canonical form. Matching ignores case and a trailing dot and covers subdomains:
excluding `example.com` also excludes `mail.example.com`. Without that, a sender could bypass a privacy
filter by using a different host under the same name. The match is label-aware, so `notexample.com` and
`example.com.evil.test` are not covered.

The participant set is every participant's `domain`; if a connector left it empty, the domain part of its
`address` is used. A malformed domain is skipped rather than failing the event: it cannot be matched
honestly, and refusing a whole page for one bad participant would stall a connector. Client and matter
values keep their case and allow 1 to 128 characters from letters, digits, `.`, `-`, `_` and `:`.

### 3. Enforcement is inside the tenant write transaction, before insert and outbox

`Store::record` and `Store::record_page` begin the tenant transaction, load the firm's excluded domains
once, and test the event before inserting. An excluded event is not inserted and no `activity.recorded`
row is appended, so nothing downstream can observe it. The check runs under row-level security in the
same transaction as the insert, so it is true for public ingest and connector paths alike and there is no
window in which an event exists before the policy is consulted.

### 4. A connector page still commits its cursor

An excluded event is counted as `excluded` in the page outcome and the runner's report; the remaining
events and the new cursor still commit. Advancing past an excluded signal is deliberate. If the cursor
stayed behind because a page contained an excluded event, the connector would re-read the same page
forever, and one privacy rule would stop capture for the whole firm.

### 5. An excluded single ingest is indistinguishable from a duplicate

`POST /v1/events` for an excluded event answers `200` with `created: false`, exactly like a retry of an
already-stored event, and `activity.recorded` is not published. The sender learns that nothing new was
stored, which it needs in order to stop retrying, but not whether the cause was a duplicate or a privacy
rule. Returning a distinct status would leak the firm's exclusion list to any connector or client that
can post events.

### 6. Management is a settings resource behind `settings:manage`

The private surface is `GET`/`POST /v1/exclusions` and `DELETE /v1/exclusions/{kind}/{value}`; the public
surface is the same shape under `/v1/privacy/exclusions`. The capture service enforces
`settings:manage`, which is the permission the vault uses for other firm-wide configuration. Adding an
exclusion is idempotent: a repeat answers `200` with `created: false` and keeps the first reason and
creation time, so concurrent admins cannot rewrite history by accident. Removing a missing exclusion is
`404`. `svc_capture` holds `SELECT`, `INSERT` and `DELETE` only; updating a reason in place is not a
supported operation. The gateway checks the kind and a conservative value shape before forwarding, so a
malformed value never reaches capture, and it mints a token for capture's audience.

A reason is optional, trimmed, at most 500 characters and free of control characters so it cannot smuggle
line breaks into an operator's log. The reason is visible to firm admins, not to end users.

### 7. The metadata-only gate is a storage rule

An event's privacy flags must agree with each other before storage:

- no content captured, no redaction claimed and no `contentRef`; or
- content captured, redaction applied and a `contentRef` of 1 to 512 characters.

Every other combination is `400` with no row written: a reference without captured content, captured
content without redaction, redaction without content, or captured content with no reference. This makes
"metadata only by default" true for every sender, including a buggy connector, without waiting for the
worker queue. The redaction worker's own tests (P0-08) remain the suite for what redaction does; this gate
is what stops unredacted content entering the event store before that worker is wired.

### 8. Exclusions are not retroactive

An exclusion governs capture from the moment it is stored. Events already in the ledger were accepted
under the policy that was in force, and history is immutable. A firm that adds an exclusion is not
promising that past work disappears; it is promising that the next signal at that domain is dropped.
Replay after removal starts from the beginning of the connector and can therefore store events that were
excluded during an earlier run, which is correct: the current policy allows them.

## Alternatives considered

- **Filter downstream in attribution or the feed.** Rejected: the event already exists, is already
  immutable, and may already have been read. A hidden row is not an uncaptured signal.
- **Delete the event after the fact.** Rejected: `svc_capture` has no `DELETE` on events and corrections
  are new events (ADR-0006). A privacy filter that edits history breaks the ledger's meaning.
- **Filter in the gateway.** Rejected: connectors and the runner call the store without a user token, so
  the gateway is not on their path. A policy enforced on one entrance is not a policy.
- **Exact domain match only.** Rejected: `mail.example.com` would bypass an exclusion of `example.com`.
  Lookalike names are still not covered, because the match is on label boundaries.
- **Case-sensitive matching, or storing the value as typed.** Rejected: `Example.COM` and `example.com`
  name the same domain; a policy that depends on spelling is a policy with holes.
- **Return 4xx for an excluded ingest.** Rejected: it tells the sender that a policy exists and which
  event tripped it, and it makes a privacy rule look like a client error to a connector. `200` with
  `created: false` keeps the existing idempotent contract.
- **A platform-wide exclusion list.** Rejected: a firm's policy is tenant data, not operator policy, and
  a global list would let one firm's exclusion affect another's capture. It also breaks RLS symmetry.
- **Store first and redact later.** Rejected: the stored event is immutable and `contentRef` may already
  have been read. The gate must run before the insert, which is why it lives in `validate`.
- **Enforce client and matter now.** Rejected: a raw signal names neither, so there is nothing to match.
  Pretending to enforce them would give admins a false guarantee; recording them now is honest and lets
  Phase 3 read the same table.
- **Delete the cursor and replay after adding an exclusion.** Rejected: it would stall on the excluded
  page forever, the exact failure rule 4 avoids.

## Consequences

- **One extra small read per write.** Each ingest and each page load reads the firm's domain list inside
  the existing transaction. The list is expected to be short and the read is indexed by tenant and kind.
  If it grows, the page path already amortises it across the page.
- **Domains are ASCII only.** An internationalised domain must be stored in its punycode form. The
  contract does not yet normalise Unicode; rejecting it is safer than a lossy comparison.
- **No tombstone for an excluded signal.** The event is absent by design, so an operator cannot later ask
  "was this mail excluded or never delivered?" The runner's `excluded` count and the capture logs are the
  available evidence. A tombstone would itself be a stored record of the excluded signal, which the
  policy exists to avoid.
- **Adding a rule does not clean the past.** Firms must be told this in the admin surface (P2-10), and the
  data-flow register should describe the from-now-on scope.
- **Policy changes are not audited yet.** The store records `createdBy` and `createdAt`, but there is no
  audit event for add or remove. That arrives with the audit chain work (P2-10/P1-14), and the table's
  shape is ready for it.
- **The redaction half of P2-07 is not deployed.** The P0-08 redaction module and its tests exist; the
  worker queue and container do not. Until they do, the metadata-only gate makes unredacted content
  impossible, and content capture stays effectively off in production.
- **Revisit** when attribution lands (client/matter enforcement), when a connector cannot supply a
  participant domain, when a firm needs exclusions scoped to one user or one source, or when policy
  changes must publish an audit event.
