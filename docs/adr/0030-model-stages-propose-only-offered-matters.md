# ADR-0030: A model may propose only an offered matter, and every run ends in a terminal state

- **Status:** Accepted
- **Date:** 2026-11-04
- **Task:** P3-07 (model stages Haiku → Sonnet → Opus, abstain and failure report)
- **Relates to:** ADR-0018 (model loop money and failure rules), ADR-0028 (attribution stages 0–1), ADR-0029 (retrieval), [research 04 §4](../research/04-autonomous-capture-design.md)

## Context

Stages 0–1 assign the certain blocks and retrieval supplies a shortlist for the rest, but neither
decides which of several plausible matters a block belongs to. Research 04 puts that decision on a
small model, repeats it on a stronger rung when the first answer is weak, and ends at a human. Three
requirements come with it: the model must be able to abstain, the stronger rung must see why the
weaker one failed, and no block may end in a silently failed state.

[`mcp-host`](../../services/mcp-host) already walks a tenant-conditional ladder, enforces a Rand cap and
maps every provider outcome to a terminal `Report`. It has no opinion about what a good reply looks
like. The attribution service needs to supply that opinion without growing a second loop or letting
provider text, prompts or replies reach a log.

## Decision

### 1. The existing loop is the ladder; this increment supplies three things

[`services/attribution`](../../services/attribution) is a library crate. It builds the prompt, gives
`run` a validator and turns the `Report` into an attribution outcome. The walking, rung skipping,
budget checks, step limit and terminal mapping stay in `mcp-host` (ADR-0018). A second loop would
drift from the first, and the first is already tested against provider failures and caps.

### 2. One reply shape, and only an offered matter is accepted

The model replies with JSON only, either `{"matterId":"<id>"}` for exactly one candidate or
`{"abstain":true}`. Unknown fields are rejected. Any other shape, including prose, a bare matter id,
both fields at once, an empty id or `{"abstain":false}`, is malformed. A reply naming an id that was
not in the candidate list is recorded as `unknown_matter` and can never become a proposal.

The validator accepts a reply only when it names a candidate the prompt offered. Acceptance is a
mechanical check, not a confidence judgment: confidence and per-tenant thresholds are P3-08. The
candidate list is deduplicated by matter id and sorted, so the same input always produces the same
prompt and the same order.

### 3. Abstain escalates, and exhausted escalation is a person

An abstain is a valid answer, not a provider failure, so it advances to the next rung instead of
stopping the run. If every rung that answered abstained, the review cause is `abstained`. If replies
were unusable instead, the cause is `unusable_reply`. A rejected key, an unreachable provider, a
spent budget, a step limit or an empty ladder keep the reason `mcp-host` already names. With no
candidates nothing is called at all and the cause is `no_candidates`.

Every input therefore returns one of two outcomes: `Proposed`, naming a candidate and the client from
our own candidate record, or `Review`, carrying the cause, the candidate matter ids and the report.
There is no error return and no panic path in the result.

### 4. The failure report is returned and also handed to the next rung

Each failed attempt is recorded as `{attempt, kind}` with kind `abstained`, `malformed` or
`unknown_matter`. A provider wrapper appends those sentences to the next rung's prompt before it
calls the real provider. That is research 04's requirement that stage 4 receives stage 3's failure
report, without changing the `mcp-host` API. The report travels in the outcome with every `Call`
(tier, outcome, usage, Rand cost) and the total spend, so the caller can persist both.

Feedback contains the failure kind only. It never repeats the model's reply, the activity text or
the prompt, so a malformed reply cannot be laundered into the next prompt through the report.

### 5. A proposal is not a post

The outcome is `Proposed`, never `assigned`. Research 04's two-key rule still applies before
anything reaches the ledger: the service confirms the matter is open, the client is not
conflict-flagged and a rate card exists. The client id in a proposal comes from the candidate we
offered, not from the reply, so a model cannot invent a client either.

### 6. The activity is framed as data, and full injection defence is P3-10

The system prompt says the text inside `<activity>` is firm data and that instructions inside it
must not be followed. A closing `</activity>` in the activity is neutralised so it cannot end the
frame. This is framing, not a guarantee. A red-team corpus with adversarial activities belongs to
P3-10; until it passes, the model remains one input to a proposal that a person or the deterministic
checks can reject.

### 7. Prompts and replies stay out of logs

The loop logs tier, token counts and cost per call (ADR-0018); this module adds no logging. The
prompt contains client labels and activity text, so it is tenant data. Replies are not persisted by
this increment and are not included in the failure report.

## Alternatives considered

- **Write a second loop for attribution.** Rejected: it would duplicate budget, rung and failure
  rules that ADR-0018 already fixed and tested, and the two would drift on the first provider change.
- **Accept free-form prose and extract a matter id with heuristics.** Rejected: every extraction rule
  is a place to guess past an abstain or an invented id. A strict shape makes an unusable reply
  visible as a failure instead of a silent best guess.
- **Stop at stage 3 when it abstains.** Rejected: an abstain is exactly the low-confidence signal
  research 04 escalates on. Stopping early would send blocks to a person while a stronger rung was
  still available and affordable.
- **Change `mcp-host` to carry the failure report in the request type.** Rejected for this increment:
  it touches every provider and adapter test for information a provider wrapper can add. If a later
  stage needs structured feedback outside the prompt, the API can grow then.
- **Post the model's choice immediately.** Rejected by the two-key rule. A model choice is one key;
  the deterministic matter checks are the other. Auto-posting without calibration is P3-08's problem,
  not this stage's.
- **Use a larger token budget or a tool call per candidate.** Rejected: the reply is one id or an
  abstain, so 128 tokens is ample, and tools would let the model act instead of answer.

## Consequences

- **Attribution gains a complete, measurable stage.** The gold set can now be run through the ladder
  once a provider is scripted or leased; the outcome and report are plain data.
- **Model quality is not claimed yet.** Precision, abstain rate and the value of escalation depend on
  a provider and on P3-08's calibration and P3-11's harness. The tests here prove the contract, not
  the model.
- **An abstain costs a stronger call.** A block every rung abstains on costs three calls before a
  person sees it. The Rand cap bounds that, and a tenant can set a lower step or budget limit.
- **Feedback can accumulate.** Each retry carries every earlier failure; with a three-rung ladder the
  prompt grows by at most two short sentences.
- **Candidate labels are tenant data.** The prompt includes the label the firm gave a matter and the
  activity text. A hosted provider sees both; that data flow must stay in the disclosure register
  (ADR-0024), and P3-06/07 do not add one.
- **Revisit** when P3-08 sets thresholds, P3-10 supplies the adversarial corpus, and the service
  gains persistence, routes and a review queue. The library interface is intended to be called
  unchanged by that service.
