# AgentOS Tasks for TimeLex

Ordered single-PR tasks mapped to [docs/agentos/PHASES.md](PHASES.md) and aligned with [docs/TASKS.md](../TASKS.md). Work in order, one small reviewed pull request per task, and append one line to [docs/agentos/REPORT.md](REPORT.md) in each PR.

## Phase 1: Research, Hygiene, and Stale-Doc Alignment
- [x] Update `README.md` status banner and repository layout to reflect implemented `matters`, `capture-hub`, `timelex-segmentation`, `timelex-attribution`, and `services/attribution` (`P3-01` to `P3-07`).
- [ ] Audit open Dependabot PRs (#51 to #54) and verify CI workflow action pin consistency across `.github/workflows/`.
- [ ] Research and document `services/attribution` HTTP routes, Postgres schema, and `mcp-host` wiring requirements in `docs/research/`.

## Phase 2: Gate A Attribution Safety, Calibration, and Evaluation
- [ ] Implement per-tenant calibration and threshold derivation with reliability metrics on the synthetic gold set (`P3-08`).
- [ ] Build the prompt-injection red-team corpus and CI tests for `services/attribution` (`P3-10`).
- [ ] Add per-call Rand cost metering and tenant spend reporting in `services/mcp-host` and `services/attribution` (`P3-12`).
- [ ] Implement the evaluation harness with Gate A release gates in `eval/` and wire it into `.github/workflows/ci.yml` (`P3-11`).

## Phase 3: Narrative Service and Number Guard
- [ ] Research ClearCut narrative overlap and record the shared-library vs service-local decision in an ADR (`P3-09`).
- [ ] Scaffold `services/narrative` with `tools/new-service`, OpenAPI contract, health/meta endpoints, and RLS schema.
- [ ] Implement deterministic narrative templates, per-client style config, and the strict number guard with adversarial unit tests.
- [ ] Add `narrative` contract examples to `crates/timelex-types` and verify `@timelex/sdk` with `npm run check:sdk`.

## Phase 4: Platform Wiring and Gateway MCP Routing
- [ ] Add `POST /mcp` routing in `services/gateway` and test end-to-end MCP tool dispatch via `crates/timelex-mcp` (`P1-15`).
- [ ] Add `services/matters` proxy routes in `services/gateway` with OpenAPI contract verification.
- [ ] Implement `capture-hub` gap detector and capture health alerts with unit and integration tests (`P2-08`).

## Phase 5: Ledger Foundation and Gate A Review
- [ ] Run the full Gate A evaluation suite, record accuracy, safety, and cost metrics in `docs/WORKLOG.md` (`G3-A`), and review findings.
- [ ] Scaffold `services/time-ledger` with append-only Postgres schema and `timelex-units` integration (`P4-01`).
- [ ] Add Postgres role-isolation tests proving `UPDATE` and `DELETE` are rejected for the `time-ledger` service role.
