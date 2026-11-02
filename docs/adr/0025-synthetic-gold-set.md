# ADR-0025: The synthetic gold set is deterministic, public and label-first

- **Status:** Accepted
- **Date:** 2026-11-02
- **Task:** P3-01 (synthetic gold-set generator)
- **Relates to:** ADR-0006 (immutable activity events), ADR-0021 (idempotent capture), ADR-0024 (data-flow disclosure), [research 04 §6](../research/04-autonomous-capture-design.md)

## Context

Gate A is a measured claim: attribution precision, recall, narrative faithfulness, terminal-state rate
and cost are reported per release, and a release is blocked when a target fails. Measurement needs
labelled work. The consented real corpus cannot be the first source: it depends on a POPIA protocol
(P3-02), lives privately, and cannot be committed. Before it exists, the only honest way to test the
pipeline is a synthetic firm whose every signal already has a correct answer.

A synthetic corpus fails in four predictable ways. It is too easy, so it measures nothing. It changes
without notice, so a regression looks like a pass. It drifts from the real event contract, so the
pipeline is measured against a shape production never sees. Or it leaks: a "fake" corpus carrying a
real mailbox, a real client name or a credential is worse than no corpus. The generator is the long
pole for the gate, so these are design constraints, not cleanup.

## Decision

### 1. One seed produces the same bytes, and CI proves it

The generator takes one integer seed and uses a single `random.Random(seed)` plus a counter-based id
source. Output is canonical JSON: sorted keys, two-space indent, one trailing LF. A manifest records
the generator version, the seed, the counts and a SHA-256 and byte length per file. The corpus is
committed. `python -m timelex_goldset check --corpus corpus` regenerates in memory and fails on a
missing, unexpected or differing file, and CI runs it. An edited corpus therefore cannot pass without
the generator changing in the same review.

### 2. It is synthetic, public and small enough to read

Every firm, user, client and matter is invented. Every address ends in a reserved domain (`.example`
or `.test`). There are no message bodies, attachments, credentials or personal data. The corpus is
public because it contains nothing private, and public because reviewers and future tests need to see
the cases. The consented real corpus remains private under P3-02 and never enters this folder.

### 3. Events are the real contract, including the id

`events.json` holds activity events that validate against
[`activity-event.schema.json`](../../contracts/events/activity-event.schema.json) at version `1.0`.
Their ids are computed with the documented layout: domain separation, length-prefixed tenant, source
system and source id, then whole-second start. The Python implementation is pinned to the same golden
value as `timelex-types`, so the two cannot drift silently. Threads and meetings are emitted as
observable source material; every mail and calendar event names the message or meeting it came from,
in both directions.

### 4. Observation and truth live in different files

`threads.json` and `calendar.json` contain only what a connector could see. `labels.json` holds the
answer, one label per event and in the same order. A label states:

- whether the signal should be stored at all (`stored` or `excluded`);
- the expected outcome (`auto_post`, `review`, `not_billable` or `excluded`);
- the correct client and matter, or `null`;
- whether it is billable;
- the evidence a correct reader can use (`hard_key`, `counterparty`, `content`, `history` or `none`);
- every matter a reader could honestly choose from the signal alone, and whether that set is
  ambiguous.

A matter reference in a subject or filename is a hard key and may auto-post. A client-only signal is
ambiguous when the client has more than one open matter, and expects review. A signal on a closed
matter always expects review even with a hard key, because the deterministic open-matter check must
catch it. Internal and newsletter signals are stored but not billable. The corpus also includes one
non-billable courtesy call that still names a client and matter.

### 5. Excluded signals are present, and labelled for dropping

`events.json` is the feed a connector would observe, so it contains mail and meetings at the firm's
excluded domains. Their labels say `excluded` and carry no client, matter or billable time. This is
deliberate: a corpus that omitted them could not prove capture drops them, and a harness that treats
every event as stored would be wrong. The README states the rule.

### 6. Metadata only, for now

Every generated event has `contentCaptured: false`, `redactionApplied: false` and no `contentRef`,
matching the platform's current gate. No message bodies are generated, so the corpus measures the
metadata stages first. The `history` evidence value is reserved but not produced, because the corpus
does not yet model assignment history. Both gaps are named rather than faked.

## Alternatives considered

- **Start with the consented real corpus.** Rejected for now: it needs the P3-02 protocol and legal
  review, is private, and cannot be a CI input. It remains the calibration source after the
  synthetic corpus proves the pipeline.
- **A hand-written fixture.** Rejected: it is small, easy and rots silently. A generator makes the
  case mix explicit and reproducible, and a seed makes a second firm cheap.
- **Generate only inside tests, never commit.** Rejected: there would be no reviewable diff, and a
  change in the case mix could pass unnoticed. The manifest and `check` make drift visible.
- **Emit a simplified event shape.** Rejected: the corpus would rehearse a contract production does
  not use, and the canonical id, privacy flags and participant rules would go untested.
- **One label: the correct matter.** Rejected: it cannot express a signal that must be dropped, work
  that is not billable, a closed matter that must not post, or a case where several matters are
  honestly possible. Those are the cases that make a precision number meaningful.
- **Omit excluded signals because capture drops them.** Rejected: the drop is the behaviour under
  test, and an absent signal cannot exercise it.
- **Generate a different firm on every run.** Rejected: unreproducible failures and unreviewable
  diffs. Different firms are a seed away when needed.

## Consequences

- **The corpus is committed and regenerated, not curated by hand.** Any generator change alters the
  manifest digests and the corpus diff in the same pull request. A reviewer can see exactly which
  case mix changed.
- **It is one firm and one 20-day window.** It is a regression corpus, not a survey of practice
  shapes. Conclusions about real firms still require P3-02.
- **Content-dependent attribution is not measured yet.** Stages 2 to 4 need text; the corpus has
  none. It measures hard keys, counterparty signals, ambiguity, exclusions and the closed-matter
  guard. Later stages extend the generator rather than replace it.
- **The harness must apply `expectedStorage`.** Treating every row in `events.json` as stored would
  count signals capture is meant to drop. That rule belongs in the P3-11 harness and in the README.
- **CI grows a Python evaluation job.** It runs ruff, pytest and the drift check on a pinned CPython
  3.12. The cost is small; the alternative is a corpus nobody verifies. The interpreter is pinned
  because the standard library guarantees only the `random()` stream across versions, not the
  higher-level helpers the generator uses.
- **Revisit** when the redaction and content path lands (bodies can be added with a redacted content
  reference), when assignment history exists (the `history` evidence), and when P3-02 supplies the
  first consented real corpus.
