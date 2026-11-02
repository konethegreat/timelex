# ADR-0022: Manual quick capture is its own resource

- **Status:** Accepted
- **Date:** 2026-11-02
- **Task:** P2-08 (manual quick-capture API and mobile page)
- **Relates to:** ADR-0006 (activity events), ADR-0011 (tenancy), ADR-0012 (internal auth), ADR-0021 (idempotent ingest)

## Context

Most work in a South African practice leaves no connector trace: a call from a client, a consultation in
chambers, a note after court. Phase 2 needs a way for a person to record that work so it becomes the
same kind of activity event as a captured email. The obvious shortcut is to let the client build an
`ActivityEvent` and post it to `POST /v1/events`. That would mean every client reimplements the
canonical id rule, chooses `source.system`, spells the firm and user, and could name a different firm
or user in the body. It would also put the id rule in a browser, where it will drift.

Two questions follow. What does a person actually submit? And where does the public form live?

## Decision

### 1. A dedicated `POST /v1/entries`, not a client-built event

The caller submits what it knows: `entryId`, `kind`, `startedAt`, `endedAt` and an optional
`description`. The capture service derives everything else: the firm and user from the verified token,
`capturedAt` from the server clock, `source.system = manual`, the canonical id, the schema version and
the privacy flags. Unknown request fields are refused.

The private path is `/v1/entries`. The gateway's public path is `POST /v1/capture/entries`; it forwards
to the service's path with a `principal` token. `/v1/events` stays private until a connector is
deployed behind the gateway.

### 2. `entryId` is the idempotency key and the source id

`entryId` must be a UUID. It becomes `source.sourceId`, so the canonical id still contains the firm,
the source system, the source id and the whole-second start. A retry after a lost response carries the
same key and the same start and therefore lands on the same row: `201` becomes `200` with
`created: false`. A different start is a different signal, which is honest because events are
immutable; a correction is a new event, never an edit.

The server does not generate the key. A key minted on the server would change on every request and
could not deduplicate a client retry.

### 3. Three kinds a person may capture

`call`, `meeting` and `manual_entry` (shown as "Note" in the form). Everything else has an automated
source; a manual claim of `email_sent` or `desktop_focus` would be a guess. The set is part of this
contract version and grows only with a reason.

### 4. The people who record work hold `capture:write`

Until this increment only the machine `agent` role held `capture:write`, because capture meant a
connector. A form for a person makes that policy wrong: a fee earner who takes a call must be able to
record it. The permission now belongs to `fee_earner` and `admin` as well; `billing` and `auditor`
still do not hold it. `capture:write` records an observed signal, not a billable time entry, so it does
not imply `time:write`.

### 5. The description is a title, not content

`description` becomes `subject.title`, with runs of whitespace collapsed to one line and a 500
character limit, the contract's existing title limit. Nothing else the person typed is stored.
`privacy.contentCaptured` is always `false` and `redactionApplied` is always `false`: quick capture is
metadata-only by construction, not by a later filter.

### 6. The form is served by the gateway, same origin

`GET /capture` returns one self-contained HTML page embedded in the gateway binary. It posts to
`/v1/capture/entries` on the same origin, so no CORS policy is needed and no second deployable is
added (Phase 2 allows four). The page loads no third-party asset; a Content Security Policy allows
inline style and script only, forbids framing and sets `form-action 'none'`. The API key is held in the
page's memory while it is open, never in `localStorage` or `sessionStorage`, never in a URL or a cookie,
and never in a log or the page's markup. A reload asks for the key again.

The page is intentionally small: three kinds, two times, one description, one button. A richer client
will replace it without changing the API.

## Alternatives considered

**Reuse `POST /v1/events` with a client-built event.** Rejected: it copies the canonical id rule into
every client, exposes internal enums, and lets a caller attempt to name another firm or user (the
service would reject it, but the request shape invites the attempt). The entry resource is a smaller
trust surface.

**Call the endpoint `/v1/time-entries`.** Rejected: a manual entry is an observed activity event, not
a billable time entry. Time entries, matters and rate cards arrive in Phase 3 and will cite events;
naming this resource for them would conflate the two.

**A separate web application on another origin with CORS.** Rejected for this increment: it adds a
CORS policy to a bearer-token API and a fifth deployable, for a form of one screen. The gateway can
serve the static page at no extra operational cost.

**Let the person choose participants or a matter.** Deferred: matters do not exist until Phase 3, and
a free-text attribution is worse than none. The event's title and times are enough for the gap
detector to start from.

## Consequences

- A quick-capture retry is safe by the same database guarantee as any other ingest
  (`ON CONFLICT (tenant_id, id) DO NOTHING`), not by a client-side cache.
- Manual entries are claims, not observations. Later attribution must treat them as such; a person can
  record a call that did not happen.
- The description is visible to later stages, so it inherits the firm's retention and access rules. It
  is metadata under the current model; the privacy pipeline (P2-07) has nothing to redact here because
  no body is stored.
- The gateway now owns one HTML asset. Keeping it dependency-free and same-origin is a rule, not a
  preference: adding a script host or an external font would require revisiting the CSP.
- Editing or deleting a manual entry is not possible in v1. Corrections arrive with the ledger, as new
  events that cite the earlier ones, consistent with ADR-0006.
