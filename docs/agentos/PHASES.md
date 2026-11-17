# AgentOS Execution Phases for TimeLex

This plan sequences the next concrete engineering increments for TimeLex, grounded in the repository's current state and the product roadmap in [docs/PHASES.md](../PHASES.md) and [docs/TASKS.md](../TASKS.md).

## Phase 1: Research, Hygiene, and Stale-Doc Alignment
Goal: Align repository documentation, dependency updates, and research notes with the implemented crates and services before adding new Gate A features.
Done when: `README.md` reflects all implemented crates and services, open Dependabot PRs and CI workflow pins are audited, `services/attribution` wiring requirements are documented, and `tools/check_links.py` passes.

## Phase 2: Gate A Attribution Safety, Calibration, and Evaluation
Goal: Complete the remaining Phase 3 (Gate A) calibration, prompt-injection red-team, Rand cost metering, and CI evaluation gate deliverables.
Done when: Per-tenant calibration (`P3-08`), red-team corpus tests (`P3-10`), Rand cost metering (`P3-12`), and the evaluation harness with CI release gates (`P3-11`) are implemented and passing.

## Phase 3: Narrative Service and Number Guard
Goal: Implement `services/narrative` with deterministic templates, per-client style configuration, and a strict number guard (`P3-09`).
Done when: `services/narrative` generates fee-ready narratives from attribution blocks, rejects any ungrounded figure with 100% number-guard test coverage, and passes `cargo test --workspace` and `npm run check:sdk`.

## Phase 4: Platform Wiring and Gateway MCP Routing
Goal: Connect built services through `services/gateway` (`P1-15`) and close remaining capture-platform gaps.
Done when: `services/gateway` routes `POST /mcp` and REST calls to `matters` and `attribution`, `capture-hub` gap detection (`P2-08`) is tested, and `cargo test --workspace` passes.

## Phase 5: Ledger Foundation and Gate A Review
Goal: Record the Gate A readiness review (`G3-A`) and establish the append-only `services/time-ledger` foundation (`P4-01`).
Done when: Gate A evaluation metrics are recorded in `docs/WORKLOG.md` and `services/time-ledger` passes Postgres role-isolation tests proving `UPDATE` and `DELETE` are rejected.
