# ADR-0028: Attribution starts with exact keys, and history never outranks one

- **Status:** Accepted
- **Date:** 2026-11-03
- **Task:** P3-05 (attribution stages 0–1: hard keys, history)
- **Relates to:** ADR-0006 (immutable activity events), ADR-0025 (synthetic gold set), ADR-0026 (deterministic segmentation), ADR-0027 (matters natural keys and import), [research 04 §4](../research/04-autonomous-capture-design.md)

## Context

Segmentation turns raw events into blocks of time. A block still says nothing about which matter earned
the time, and posting it to a guess is worse than posting nothing: a wrong reference attaches
unbillable work to a client and misprices it. Attribution is therefore a cascade of stages, each of
which may name one matter, abstain, or ask a person. P3-05 is the first two stages:

- **Stage 0, hard keys.** The signal itself contains an exact name from the firm's records: a matter
  reference in a subject or filename, a client's email domain, a document workspace or a calendar tag.
- **Stage 1, history.** The same thread, document or participant set was already assigned earlier in
  the same run.

The gold set (`eval/synthetic/corpus`) gives every stored signal a correct client, matter and expected
outcome. Its evidence labels are `hard_key`, `counterparty`, `content`, `history` and `none`; P3-05
must honour the first two and must never contradict any label, including one that expects review.

Three failure modes shape the design. A substring match can find `M-2026-0101` inside
`M-2026-01010` and claim the wrong matter. A counterparty group can hold work on several matters for
one client, so a single reference on one event must not speak for the group. And history can be stale:
a conversation that changes matter would otherwise keep posting to the old one with no visible
signal.

## Decision

### 1. Stage 0 matches whole references, case-insensitively

[`timelex-attribution`](../../crates/timelex-attribution) takes a validated `Directory` snapshot of
clients, matters, references and aliases (ADR-0027) and searches each event's `title`, `thread_ref`
and `document_ref` for every reference the directory knows, including explicit `matter_reference`
aliases. Matching lower-cases both sides and requires a non-alphanumeric boundary before and after
the match, so `M-2026-0101` resolves in `[M-2026-0101] lease` and `re: m-2026-0101` but not in
`M-2026-01010`, `XM-2026-0101` or `M-2026-0101A`. A reference that matches no matter is not a key.

Every text field of every event in the group is searched. Repeated references to the same matter
collapse into one match; two different matters are `ConflictingHardKeys`, and the review lists both.

### 2. A thread or document speaks for its group; a counterparty group must speak for itself

Segmentation builds three shapes. When every event in a group shares one thread, a reference on any
event describes the conversation, so it applies to the whole block. The same holds for a shared
document. A counterparty group has no such identity: it can contain two meetings or calls with the
same client about different matters. For that shape, every event must carry the same reference.
If only some events do, the decision is `MixedEvidence` and the candidates are the referenced matter
plus the client's other open matters. One event's reference never assigns another event's time.

### 3. A client domain names a client, not a matter

A participant domain from the directory's `client_domain` aliases resolves the client. It assigns a
matter only when that client has exactly one open matter. Otherwise the block is `AmbiguousClient`
with the open matters as candidates, or `ClosedMatter` when the client has no open matter. With three
open matters per client in the gold set, every counterparty-only signal correctly lands in review
rather than posting to an arbitrary one. Two or more clients in one group are `MultipleClients`.

### 4. Checks that protect a hard key run before it may post

A resolved reference is not sufficient on its own. The block is reviewed when:

- the matter is closed (`ClosedMatter`), even though the reference is exact;
- the participants name a different client than the reference (`ConflictingHardKeys`);
- the participants name more than one client (`MultipleClients`);
- a counterparty group carries the reference on only some events (`MixedEvidence`).

Only then does stage 0 return `Assigned { evidence: MatterReference or ClientDomain }`.

### 5. History splits by strength: a thread or document names a matter, participants name a client

`History` records only `Assigned` decisions; reviews and unattributed groups teach nothing. A
remembered thread or document maps to a matter: one conversation or file belongs to one piece of
work. A remembered participant set maps to a client only, because the same contact appears across
all of that client's matters; resolving it runs the same one-open-matter rule as a domain key. A
remembered matter that has since closed is `ClosedMatter`; two remembered matters or clients are
`ConflictingHistory`. History is consulted only after stage 0 abstains, so a hard key always
outranks what was remembered.

### 6. Abstention is a result, not an error

`Decision` has three variants: `Assigned`, `Review` with a reason and sorted, de-duplicated candidate
matter ids, and `Unattributed`. An unattributed group is not a failure; it is the input to the
retrieval and model stages. A review names every matter the evidence can honestly support, so the
person choosing is not shown a misleading single option. The library does no I/O, reads no clock,
uses no floats, and does not persist anything. The caller supplies the directory and the history.

### 7. Measured against the gold set, precision is perfect and recall is partial

`tests/gold_set.rs` filters the corpus to stored events, groups them by actor, segments with the
default configuration, attributes each block in chronological order and records assignments. With
the committed corpus:

- 208 stored events became 193 time blocks: 102 assigned, 71 review and 20 unattributed;
- 102 of 102 assignments named the labelled matter: 0 wrong matters, 0 false positives, 100%
  precision;
- all 101 auto-post labels that claimed time were assigned their labelled matter; 8 more claimed no
  time and are accounted for separately;
- 75 review-labelled signals reached review, with 0 candidate misses and 0 wrongly auto-posted;
- 7 content-dependent signals were left for later stages;
- every assignment used `MatterReference`; the corpus does not model assignment history (ADR-0025).

Precision is the gate here. A stage that buys recall with a wrong matter is worse than one that
abstains, because the wrong matter posts and the firm may not notice. Recall is expected to rise in
P3-06 and P3-07, which add retrieval and model stages behind review.

## Alternatives considered

- **Match references anywhere in the text.** Rejected: `M-2026-01010` would claim `M-2026-0101`, and a
  wrong post is silent. Whole-token matching loses a few real matches that a person would want, and
  those fall through to later stages instead.
- **Let any reference in a counterparty group assign the whole block.** Rejected: counterparty groups
  intentionally gather work with one client, which may span matters. The gold set mixes such signals,
  and a single reference would misattribute the rest of the block.
- **Assign any known client domain to one of the client's matters.** Rejected: picking among three
  open matters is a guess. `AmbiguousClient` with candidates is honest and cheap for a person.
- **Use participant history to name a matter.** Rejected: contacts recur across matters, so the
  remembered association would be a guess dressed as history. Participant history names a client and
  then applies the open-matter rule.
- **Consult history before hard keys.** Rejected: a stale thread assignment would override a subject
  that names the new matter. Hard keys are the stronger evidence and come first.
- **Learn from reviews, or record a review as history.** Rejected: a review is a decision a person has
  not made yet. Recording it would turn an open question into a remembered fact.
- **Return one best candidate instead of a list.** Rejected: it hides the ambiguity from the reviewer
  and invites rubber-stamping. The review names every matter the evidence supports.
- **Ship stage 0 as a service now.** Deferred: persistence, the review queue and the gateway route
  belong with the harness and ledger work (P3-11 and Phase 4). A pure library can be measured today
  without inventing a database shape for blocks before their consumer exists.
- **Fuzzy reference matching and learned aliases.** Deferred: fuzzy matching is a later stage with its
  own precision budget, and learned aliases need human approval (ADR-0027).

## Consequences

- **The gate's first number exists.** Attribution precision on auto-posted blocks is measured at 100%
  against the committed corpus, with the corpus and harness checked in CI. Later stages can only
  lower it by adding evidence, and the test will catch a contradiction with any label.
- **Recall is deliberately low.** 20 blocks are unattributed and 71 need a person at this stage. That
  is the expected shape before retrieval and model stages, not a defect to hide.
- **15 stored events claimed no time.** Segmentation clips events fully covered by an anchor, so some
  signals never form a block and cannot be attributed. The test reports this instead of counting it
  as an attribution error.
- **History is not measured by the corpus yet.** Only unit tests exercise it, because the generator
  does not model prior assignments. When P3-02 or the harness supplies real history, the same
  interface applies; the `history` evidence label becomes meaningful then.
- **Reference matching is linear in directory references per text field.** With 24 matters this is
  trivial. A firm with tens of thousands of references will need an index or a different matcher;
  revisit before the directory grows large.
- **The directory is a point-in-time snapshot.** A caller must rebuild it when matters, references or
  aliases change. The library cannot see writes made after the snapshot, which is acceptable for a
  batch run and must be stated by whichever service owns the snapshot.
- **Review reasons are part of the API.** The review queue and UI will render them; renaming one is a
  contract change for those consumers, so they are kept few and explicit.
- **Revisit** when retrieval and model stages land (P3-06/P3-07), when content signals can be
  measured (with redacted bodies), when assignment history exists in the corpus, and when the
  directory outgrows a linear scan.
