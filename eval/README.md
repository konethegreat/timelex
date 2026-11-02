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

What it deliberately tests: hard keys (matter references), client-only signals that are ambiguous across open matters, closed matters that must not auto-post, excluded domains that must never be stored, internal and newsletter signals that are stored but not billable, and metadata-only privacy flags everywhere. What it does not: message bodies (content-dependent stages), assignment history, or any real firm. Those arrive with P3-05/P3-06 and the P3-02 protocol.

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
