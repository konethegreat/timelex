# ADR-0031: Confidence is calibrated per tenant, and the ladder threshold is derived from measured precision

- **Status:** Accepted
- **Date:** 2026-11-20
- **Task:** P3-08 (calibration and per-tenant threshold derivation)
- **Relates to:** ADR-0005 (autonomy ladder and review), ADR-0028 (attribution stages 0 and 1), ADR-0030 (model stages), [eval README](../../eval/README.md)

## Context

The ladder in ADR-0005 auto-executes a decision only when its confidence is at least a
tenant-specific threshold. Today a decision carries an evidence class, not a probability: stages 0
and 1 report the kind of key that matched, and the model stages either name an offered matter or
abstain without stating a probability. A class name cannot be compared with a 98% precision target,
and the same class means different things at different firms.

The Gate A target in the eval README is attribution precision at threshold of at least 98%. To
operate the ladder we need three measured things per tenant: a map from a stage's raw score to the
rate at which that score was correct, an error measure that says the map is honest, and the lowest
score whose selected set still holds the precision floor.

## Decision

### 1. Calibration is a pure library with a narrow input

[`timelex-calibration`](../../crates/timelex-calibration) accepts observations of (raw score in
[0, 1], correct or not) and nothing else. It reads no clock, touches no network, holds no model and
stores no data. Floats live here, not in `timelex-attribution`, so the attribution crate keeps its
no-floats charter and stage 0 and 1 stay exact. A per-tenant fit takes only that tenant's
observations.

### 2. The map is monotone and fitted without a solver

The map is fitted with pool-adjacent-violators (isotonic regression): sort by score, average
neighbouring groups while the frequency of a higher score would fall below a lower one, and carry
the pooled frequency for every score in the block. The result never decreases as the score rises, so
calibration cannot invert the order the stage already trusts, and a score between two fitted steps
takes the lower step.

### 3. Reliability is measured over ten bins

Observed correctness and mean confidence are reported in ten equal-width confidence bins: the
expected calibration error (ECE, weighted by bin population) and the Brier score (mean squared
distance between score and outcome). A text diagram renders one row per occupied bin with observed
and predicted bars so a report can carry the picture without a plotting dependency. The top bin
includes 1.0.

### 4. The threshold is the lowest score that holds the floor

The derived threshold is the lowest score whose selected set (every observation at or above it)
keeps measured precision at or above the floor, so recall is as high as the floor allows. Tied
scores enter or leave the selected set together. When no score reaches the floor, derivation
returns none and the ladder cannot auto-execute. The floor stays a per-tenant, per-decision-type
policy setting capped by system rules (ADR-0005); the first floor is 0.98 to match the Gate A
target, and changing it requires a WORKLOG entry.

### 5. Stage 0 and 1 scores are the evidence classes

Until a stage reports its own probability, the raw score for an assignment is the nominal
confidence of its evidence class: matter reference 0.95, client domain 0.90, history thread 0.85,
history document 0.80, history client 0.70. Model stages contribute no observations today because
they do not return a probability; their scores arrive with the first stage that does, and the fit is
refitted when a stage changes.

### 6. The gold-set run is a committed test, and the harness comes later

The measurement is a test over `eval/synthetic/corpus`, in the same style as the attribution
gold-set test: it segments, attributes in time order, builds observations, fits, measures, derives
the threshold and prints the diagram, the threshold, the recall and the coverage. It asserts the
invariants (a monotone map, Brier never rising, a threshold that holds the floor and selects exactly
the scores at or above it). The rendered diagram is recorded in the WORKLOG until P3-11 writes
`eval/reports`.

## Measured on the gold set (2026-11-20)

Tenant `tn_gb911f19670423736`: 208 stored events, 192 labelled with a matter, 102 assigned blocks
(all through an explicit matter reference). Raw ECE 0.050 against an observed rate of 1.000;
calibrated ECE 0.000 and Brier 0.000; threshold 1.000 at the 98% floor, selecting all 102 events
with recall 0.531 of labelled matter events and coverage 0.490 of stored events.

The result is a genuine null measurement: the corpus exercises only the evidence class that never
errs, so calibration has nothing to correct and the threshold collapses to that class. That is
recorded rather than hidden: the accuracy of the map matters once model stages or history produce
imperfect scores, and the crate is tested against overconfident and pooled samples for that case.

## Alternatives considered

- *Platt scaling or logistic calibration.* Needs a convex solver as a dependency and hides the
  shape of the correction; isotonic needs neither and shows the fitted steps.
- *Histogram binning.* The bin estimates can invert the order of two nearby scores, which the
  ladder would read as a stronger decision at a lower score.
- *One global threshold.* Rejected already in ADR-0005: risk differs by tenant, client and amount.
- *Let the model state its confidence.* Model-reported confidence is poorly calibrated and puts
  policy inside the model (ADR-0005, alternatives).

## Consequences

- The threshold is only as good as the labelled sample behind it. The gold set holds one tenant, so
  per-tenant effects are unmeasured until the consented corpus (P3-02) exists.
- A correction to a posted entry must join the tenant's next fit; a failed audit sample demotes the
  tenant (ADR-0005) because the threshold is re-derived from worse observations.
- Model stages cannot raise autonomy on their own until they emit a score; until then the ladder can
  only trust stages 0 and 1.
- Adding a metric or changing the bin count changes the reported numbers and needs a WORKLOG entry.
