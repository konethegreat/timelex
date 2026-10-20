# eval/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Accuracy is **measured, not asserted** ([research 04 §6](../docs/research/04-autonomous-capture-design.md), [AUTONOMY.md](../docs/AUTONOMY.md)). This folder will hold the evaluation harness and the public synthetic gold set. Nothing is built yet (tasks P3-01, P3-11).

## Planned layout
```
eval/
  synthetic/      generator + generated corpus (public, deterministic seed)
  redteam/        prompt-injection cases (public)
  harness/        Python runner: loads a corpus, runs the pipeline, writes a report
  reports/        generated reports (gitignored except the latest release report)
```
The **consented real corpus** never lives in this repository. It is stored privately under a protocol approved after legal review (P3-02).

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
