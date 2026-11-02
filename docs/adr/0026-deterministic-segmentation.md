# ADR-0026: Segmentation is deterministic and never double counts a second

- **Status:** Accepted
- **Date:** 2026-11-02
- **Task:** P3-03 (segmentation to time blocks)
- **Relates to:** ADR-0005 (autonomy ladder), ADR-0006 (immutable activity events), ADR-0025 (synthetic gold set)

## Context

Attribution does not operate on individual events. A week of mail, meetings, document edits and calls
is noisy: one conversation produces several events, one document is open across a call, and two
signals can describe the same ten minutes. Before a model is asked which matter the work belongs to,
the events must become a small set of time blocks that a person could recognise as "what I did".

Three properties decide whether this stage is safe. First, it must be **deterministic**: the same
events must always produce the same blocks, or two runs of the pipeline disagree about billable time.
Second, it must **never credit the same second twice**: a call during document work is ten minutes of
calling, not ten minutes of calling plus ten minutes of drafting. Third, it must be **explainable**:
every block cites the events it came from, so a fee earner can see why the time exists.

Research 04 §3 already fixes the shape: group events that belong together by thread, document or
counterparty within a gap tolerance; use calendar events and calls as hard anchors; resolve overlaps
in Rust, not with a model; return start, end and duration in seconds. Billing-unit rounding is a
separate concern owned by `timelex-units`.

## Decision

### 1. A pure crate, not a service

[`crates/timelex-segmentation`](../../crates/timelex-segmentation) takes a slice of contract events and
a configuration value and returns blocks. It performs no I/O, reads no clock, calls no model and uses
no floating point. It therefore belongs to the same class as `timelex-units` and `timelex-types`: a
tested library that any service, the evaluation harness or a replay tool can call. The attribution
service will own persistence, ids and events around it, but not the arithmetic.

### 2. One tenant and one actor per call

A block is one person's time. Mixing actors would let a shared thread become a single block that bills
two people, and mixing tenants would cross a tenancy boundary. A call with more than one tenant or
actor is refused (`MixedTenants`, `MixedActors`), so the caller must group first. Empty input returns
no blocks; it is not an error.

### 3. Whole-second, half-open intervals

A block covers `[start, end)` in whole Unix seconds. The start is rounded down and the end up, so an
event that touches any part of a second claims that second. Provider timestamps are not more precise
than that, and rounding outward never loses time. An event whose end precedes its start is refused
with its id (`EndBeforeStart`); a point event (end equal to start) claims no time and produces no
block, though it remains in the ledger.

### 4. Grouping has one priority order

An event joins a group by the strongest reference it carries:

1. `subject.threadRef` (a conversation);
2. `subject.documentRef` (a file);
3. the sorted, lower-cased, de-duplicated participant domains (a counterparty set);
4. otherwise its own event id, so it stands alone.

References are trimmed and an empty reference falls through to the next level. Domains are
case-insensitive and a trailing dot is ignored. The fallback matters: two unrelated document edits by
the same person within five minutes must not merge merely because they are both "work".

Two events in the same group join when the gap between their intervals is at most the configured
tolerance (300 seconds by default). The boundary is inclusive. Merging also joins overlapping
intervals within a group; the block spans their union.

### 5. Anchors claim time before background work

Kinds split into two classes:

- **Anchor**: `meeting`, `call` and `manual_entry`. The calendar, the call log or the person asserted
  that this time happened.
- **Passive**: mail, document edits and views, web research and desktop focus. These are inferred
  background activity.

Groups are placed in class order, anchors first, then by their first start and finally by group key.
Each group's merged intervals are added to a claimed union after they are placed, and every later
group is clipped against that union. A call inside a document block therefore splits the document
block into the time before and after the call. A passive group wholly covered by an anchor produces no
block at all: its time is already represented by the anchor.

Clipping rather than allowing overlap is the point. If two anchors overlap, the earlier one keeps the
shared minutes and the later one is trimmed. The output contains no two blocks that share a second,
and the sum of block durations equals the length of the union of all input intervals. That equality is
tested on fixed fixtures, on a generated adversarial set, and on the 220-event gold set.

The cost is an arbitrary tie-break between groups of the same class that start at the same second
(the group key decides) and between two passive groups that overlap (the earlier start wins). It is a
stated rule, not a random one, and the losing events remain in the immutable ledger. A later overlap
report can surface them without changing what was billed.

### 6. Evidence is the events that touch the block

Each block lists the sorted ids of the events whose intervals intersect it. After a split, the block
before the call does not cite an event that only happened after it, while an event spanning the call
appears in both blocks. Ids are enough for the pipeline to fetch the immutably stored events; the
crate does not summarise, redact or narrate.

### 7. Output is ordered and input order is irrelevant

Groups are held in a `BTreeMap`, groups are sorted by `(class, first start, group key)`, and blocks are
sorted by `(start, end, class, group, event ids)`. Reversing or shuffling the input therefore produces
the identical vector. No hash iteration order, clock or thread takes part.

### 8. Rounding is not segmentation

The crate reports exact seconds. The minimum increment, billing unit and rounding rule are applied
later by `timelex-units` when a block becomes an entry under a firm's or client's policy. A block that
is shorter than a unit still exists; whether it is billed, and as how many units, is a separate
decision with its own tests.

## Alternatives considered

- **Let a model segment.** Rejected: segmentation decides how much time is billed. A model would make
  the same input produce different blocks, could not be diffed in CI, and would move money-affecting
  arithmetic outside deterministic code (ADR-0005).
- **Allow overlapping blocks and resolve at billing.** Rejected: two blocks could each be posted for
  the same minutes unless a second stage re-implemented the same clipping, and the ledger would hold
  contradictory evidence of how long the work took.
- **Pick a winner per event kind instead of per group.** Rejected for now: a document view inside a
  document edit's own thread is the same work and should not split it. Class-level priority expresses
  the rule that matters (hard time beats inferred time) without inventing per-kind precedence.
- **Fold anchors into the nearest passive block.** Rejected: a meeting would lose its hard start and
  end, and a call would silently extend into unrelated drafting.
- **Round to six-minute units inside segmentation.** Rejected: rounding depends on the fee agreement
  and is already tested in `timelex-units`; doing it here would apply one firm's policy to every
  tenant and lose the exact interval that evidence and review need.
- **Millisecond intervals.** Rejected: no source supplies reliable sub-second work boundaries, and
  outward rounding to whole seconds is both honest and sufficient for billing.
- **Make the gap tolerance per tenant now.** Not yet: the value is a parameter, and per-firm settings
  arrive with the matters/configuration work (P3-04). The default is stated in code and tested at its
  boundary.
- **Incremental segmentation as events arrive.** Deferred: an incremental algorithm must revise blocks
  when a late event lands, which is a different design. Until then, blocks are derived data and are
  recomputed from the current event set.

## Consequences

- **Late events rewrite boundaries.** A block is a projection of the events known at the time. Replay
  or a late connector delivery can change its start, end or evidence, so consumers must treat blocks
  as derived and version or replace them rather than patching them in place.
- **Equal-class overlaps lose time for the later group.** The earlier group's block is retained; the
  later group is trimmed or, if wholly covered, absent. The events remain, so a future review surface
  can show "two documents were open at once" without double billing.
- **A call splits a document block into two blocks.** Attribution must therefore allow several blocks
  from one thread to share a matter; it cannot assume one block per thread per day.
- **Gaps stay gaps.** Travel between meetings, idle time and lunch are not invented. If a firm wants
  travel time, that is a future, explicit rule.
- **No persistence or bus event yet.** The crate returns values. Block ids, the `attribution` schema,
  `block.created` and the REST/MCP surface arrive with the attribution service (P3-04/P3-05).
- **The gold set is the first consumer.** A Rust test segments `eval/synthetic/corpus/events.json` per
  actor and checks disjointness, union coverage, evidence and input-order independence, so a change to
  the event contract or the rules fails in both languages.
- **Revisit** when per-firm gap tolerance or travel time is requested, when attribution needs an
  overlap report, when blocks must be produced incrementally, or when a connector supplies
  sub-second or timezone-changing signals.
