# eval/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Accuracy is **measured, not asserted** ([research 04 §6](../docs/research/04-autonomous-capture-design.md), [AUTONOMY.md](../docs/AUTONOMY.md)). The synthetic gold set is built ([P3-01](../docs/TASKS.md), [ADR-0025](../docs/adr/0025-synthetic-gold-set.md)); the harness, red-team corpus and reports are still planned (P3-11, P3-10).

## Layout
```
eval/
  synthetic/      generator + committed corpus (public, one seed, byte-checked in CI)  ✅
  redteam/        prompt-injection cases (public)                                      planned
  harness/        Python runner: loads a corpus, runs the pipeline, writes a report    planned
  reports/        generated reports (gitignored except the latest release report)      planned
```
The **consented real corpus** never lives in this repository. It is stored privately under a protocol approved after legal review (P3-02).

## The synthetic corpus

[`synthetic/`](synthetic) holds one fictional firm: 6 users, 8 clients, 24 matters, 50 mail threads, 38 meetings and 220 activity events over 20 business days. Every event is a valid `1.0` activity event with the canonical id, and every event has a label for client, matter, billability, storage decision, expected outcome and honest candidates. A manifest pins the seed, counts and file digests. `python -m timelex_goldset check --corpus corpus` regenerates in memory and fails on any drift, and CI runs it.

What it deliberately tests: hard keys (matter references), client-only signals that are ambiguous across open matters, closed matters that must not auto-post, excluded domains that must never be stored, internal and newsletter signals that are stored but not billable, and metadata-only privacy flags everywhere. What it does not: message bodies (the model stages exist as a library but the corpus is checked without a provider), assignment history, or any real firm. The model-stage contract landed with P3-07 ([ADR-0030](../docs/adr/0030-model-stages-propose-only-offered-matters.md)); its precision and abstain rate need the calibration and harness in P3-08/P3-11. Assignment history arrives with the P3-02 protocol.

The corpus is also consumed outside Python: [`timelex-segmentation`](../crates/timelex-segmentation) reads `synthetic/corpus/events.json` in its Rust tests and segments all 220 events per actor ([P3-03](../docs/TASKS.md), [ADR-0026](../docs/adr/0026-deterministic-segmentation.md)), so an event-contract change or a segmentation rule change fails in both languages. [`timelex-attribution`](../crates/timelex-attribution) reads the same corpus end to end, filtering to stored events, segmenting per actor and attributing every block in order. Its committed run is the first measured gate number: 208 stored events became 193 blocks, 102 were assigned and all 102 matched their label (100% precision, 0 wrong matters, 0 false positives), 71 went to review with no candidate misses, 20 were left unattributed for later stages, and no review-labelled signal auto-posted ([P3-05](../docs/TASKS.md), [ADR-0028](../docs/adr/0028-attribution-stages-zero-one.md)). Retrieval ([P3-06](../docs/TASKS.md), [ADR-0029](../docs/adr/0029-retrieval-local-embeddings-per-tenant-namespace.md)) is not measured here: the corpus has no retrieval labels and its candidate threshold belongs to P3-08, so its gate was namespace isolation rather than recall.

## Metrics (reported per release)
| Metric | Meaning | Gate A target (proposed) |
|---|---|---|
| Silent critical errors | Wrong client or amount posted without review | 0 |
| Attribution precision at threshold | Correct matter among auto-posted entries | ≥ 98% |
| Recall of billable time | Recovered minutes versus the fee earner's final submitted time | ≥ 90% |
| Narrative faithfulness | Narratives passing the number guard | 100% |
| Terminal-state rate | Jobs ending `done` or `needs_review` | 100% |
| Red-team pass rate | Injection cases that fail to change the outcome | 100% |
| Cost per entry | Rand, by model tier | reported |
| Reviewer minutes per day | Exceptions workload per fee earner | ≤ 5 (median) |

Targets are proposals to be calibrated on the first run; changing them requires a WORKLOG entry.
