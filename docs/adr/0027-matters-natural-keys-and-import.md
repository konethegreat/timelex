# ADR-0027: Matters keep the firm's own keys, and CSV import is all-or-nothing

- **Status:** Accepted
- **Date:** 2026-11-03
- **Task:** P3-04 (minimal matters, clients, aliases, rate cards, CSV import)
- **Relates to:** ADR-0001 (contract-first), ADR-0006 (immutable activity events), ADR-0009 (practice-system interop), ADR-0011 (tenancy and database roles), ADR-0012 (internal authentication), ADR-0025 (synthetic gold set), ADR-0026 (segmentation)

## Context

Attribution needs a source of truth before it can name anything. A work signal carries a domain, a
counterparty, a subject line or a document title; to turn that into "this is Meridian's lease
matter", the platform must know the firm's clients, its matters, the other names those clients and
matters go by, and what each matter costs. P3-05 measures precision against the gold set, whose
labels use the firm's own client and matter identifiers (`cl_…`, `mt_…`), so the storage model must
accept those identifiers unchanged. P1-02's acceptance criterion is import from CSV.

A law firm already has these records in some other system, usually with numbering it has used for
years. People write that numbering on letters and in email subjects. Any design that invents a new
public identifier forces every import and every future integration to maintain a translation table,
and any design that treats a display name as the key breaks the first time a client renames itself
or spells a domain with a capital letter.

The data is billing-adjacent: a wrong reference can attach unbillable time to a matter, and a wrong
rate can misprice it. Import errors are therefore not a cosmetic concern.

## Decision

### 1. The firm's identifiers are the keys

A client is stored under `(tenant_id, client_id)` and a matter under `(tenant_id, matter_id)`, where
the identifiers are the firm's own: 1 to 64 characters from `A-Z a-z 0-9 . _ : -`. There is no
surrogate public id for either. The pair is tenant-scoped, so two firms may both use `cl_1`.

The gold set, a practice-system export and a future connector can therefore import and look up
records without a mapping. An alias row keeps a UUID because the same canonical value may exist
under several kinds; that id is never the way a client or matter is addressed.

### 2. A matter reference has a canonical, unique key

`reference` is what people write on documents. Its canonical form is the trimmed, lower-cased value,
stored as `reference_key` and unique within the firm. `M-2026-0101`, `m-2026-0101` and ` M-2026-0101 `
are the same reference; a second matter claiming any of them is a 409. Uniqueness is enforced by the
database as well as by the store.

A matter's `matter_id` is separate from its reference. Renaming a reference is an update of the same
matter, so history stays attached; whether the old reference continues to resolve is an explicit
alias decision, not an accident of storage.

### 3. Aliases are explicit hard keys with one target each

Three kinds are stored: `matter_reference` (a reference to a matter), `client_name` (a display name
to a client) and `client_domain` (an email domain to a client). Values are canonicalised on the way
in: trimmed, lower-cased, and for a domain one trailing dot is removed. `(tenant_id, kind, value)` is
unique, so a value names exactly one target. The same value for the same target is idempotent; the
same value for a different target is a 409. `matter_reference` requires a matter and the client kinds
must not name one, which the schema enforces with a check constraint and a composite foreign key.

Creating a matter does not invent aliases. Automatic aliases would go stale when a name or reference
changes, and a stale hard key is worse than an absent one: it silently attributes work to the wrong
matter. A matter's `reference_key` always resolves without an alias row.

### 4. One rate card per matter

`matters.rate_cards` is keyed by `(tenant_id, matter_id)`. A card holds an hourly rate in whole ZAR
cents, the minutes in one billing unit (default 6), the rounding rule (`up`, `down`, `nearest`;
default `up`) and the minimum units for a non-zero duration (default 1). `PUT` replaces the card.
Matter-specific discounts, client defaults, fixed and capped fees are later work (P5-01); storing
an effective-dated series now would add a versioning problem before there is a consumer for it.

### 5. Reads and writes carry the service's own permission check

Every request arrives with a gateway-signed internal token (ADR-0012). Reads require a `principal`
token holding `time:read`; writes and imports require `settings:manage`, which is human-only
(AUTONOMY.md). The firm comes from the token. No request body may name a tenant, and the schema's
row-level security policy (ADR-0011) confines every query even if the store has a bug.

### 6. CSV import is strict, bounded and atomic

One file holds one record kind. The body must be `text/csv`, at most 2 MiB, with a header row and at
most 5 000 data rows. Headers are matched case-insensitively; an unknown column is refused rather
than ignored, and a missing required column is named. Blank lines are ignored and rows are numbered
logically (header is row 1, blank lines are not counted). A row with the wrong number of values is
refused.

Every row goes through the same validation as a single request, then is upserted by its natural key
inside one transaction. If any row fails, nothing is stored, and the response names the logical row
without echoing a cell value. Re-uploading a corrected file is safe because upserts are idempotent.

The bound and the single transaction are deliberate. A firm importing ten thousand matters must split
the file, which keeps the window in which other writes wait behind the import predictable. A partial
import that reports "2 000 of 5 000 applied" would leave the firm unsure what exists.

### 7. The gateway route and the outbox events wait

The service listens on the private interface and the gateway does not route to it in this increment.
Routing is per path in the gateway (ADR-0012), so adding paths is a gateway change with its own
tests, and the public contract should land together with the deploy that fronts the service.

Likewise, `matter.opened` and `matter.closed` are not published yet. They belong in an event contract
with frozen examples (the rule in `contracts/events/README.md`), and nothing consumes them until
attribution exists. The store's current writes are not event-sourced; adding events later is a
separate change, not a reason to widen this one.

## Alternatives considered

- **Surrogate UUIDs with an `externalId` column.** Rejected: it adds a translation table to every
  import and every later attribution lookup, and the gold set would need mapping before P3-05 could
  use it. The firm's identifiers are already stable and unique in their source system.
- **Use the reference as the matter key.** Rejected: references change (a matter is renumbered), and
  the key must survive that. Keeping `matter_id` separate and `reference_key` unique preserves both
  stability and the uniqueness people expect.
- **Case-sensitive references and aliases.** Rejected: people and mail systems do not agree on case,
  and a hard key that misses `Meridian.Example` because it stored `meridian.example` would silently
  fail to attribute. Canonicalisation is the point of a hard key.
- **Auto-create aliases from names, references and domains on write.** Rejected: automatic aliases
  turn every rename into a stale key that may conflict, and there is no owner to decide which name
  should win. Explicit aliases with a conflict are visible and correctable.
- **Several rate cards with effective dates.** Deferred: no consumer yet, and the versioning and
  overlap rules need their own decision with P5-01's fee arrangements. One card is honest for now.
- **Skip bad import rows and report them.** Rejected: a partial import leaves the firm's records in
  an unknown state, and the caller cannot safely retry. All-or-nothing with row numbers is slower to
  fix but unambiguous.
- **Import through a JSON batch endpoint.** Rejected: the acceptance criterion is CSV, firms export
  CSV, and a CSV parser keeps the wire body small. JSON batch can be added later without changing
  the store's upsert methods.
- **A generic gateway prefix proxy.** Rejected for now: the gateway routes explicitly per path so a
  new service cannot become reachable by accident. A prefix proxy would change that security property
  for every service.
- **Publish `matter.opened`/`matter.closed` now.** Deferred: no event contract, no consumer, and the
  open/close operations do not yet exist as separate commands. Emitting on every upsert would publish
  noise rather than facts.

## Consequences

- **Attribution can map the gold set directly.** Labels name `cl_…` and `mt_…`, which are the stored
  keys. P3-05 can resolve a candidate without a translation layer, and the import test proves all 24
  matters and 32 aliases arrive.
- **A renamed reference needs an alias if the old name must still resolve.** That is a visible,
  intentional act. A reference that was never aliased simply stops resolving, rather than pointing at
  the wrong matter.
- **Alias conflicts surface at import time.** A client domain shared by two clients (for example a
  group with several entities) is refused by row number; the firm must choose which entity owns the
  domain or leave it unaliased.
- **Imports are bounded.** Files over 5 000 rows or 2 MiB must be split. The transaction is one unit
  of work, so the service is not a streaming loader.
- **No public surface yet.** The gateway route, the public contract version, and `matter.opened` /
  `matter.closed` arrive with the deploy increment. Until then the service is only reachable on the
  private interface.
- **Rate cards are one per matter.** Discounts, client defaults and fee arrangements will need either
  a second table or a versioned card; this decision does not pretend to model them.
- **Revisit** when the gateway fronts the service, when P5-01 adds fee arrangements, when contracts
  need dated rate history, when attribution wants aliases learned from evidence (with human approval),
  or when a connector can import directly instead of through CSV.
