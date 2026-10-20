# Changelog

All notable changes are recorded here. Format based on [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/) (0.x = anything may change).

## [0.1.0] - 2026-10-20
### Added
- Research: GhostPractice teardown, competitive landscape, South African compliance, autonomous-capture design, MCP host and bring-your-own-key economics, strategy and positioning (`docs/research/01–06`).
- Architecture decision records ADR-0001 to ADR-0010, vision, architecture, autonomy ladder, threat model, glossary, phases and task backlog.
- Contracts v0: activity-event JSON Schema and golden example, gateway OpenAPI, MCP tool catalogue (draft).
- `timelex-units` (billing units and fee maths, 14 tests plus a doc test), `timelex-types` (contract types validated against the schema, 7 tests), `timelex-gateway` (health and metadata, 3 tests).
- `workers/py-worker` redaction module (19 tests) and `@timelex/sdk` TypeScript stub (3 tests).
- Specifications for twelve services under `services/`.
- GitHub project hygiene: CI for Rust, Python and TypeScript, CodeQL, dependency review, OpenSSF Scorecard, Dependabot, label sync, issue and PR templates, CODEOWNERS, security policy, governance.

### Notes
- Pre-alpha. No capture, attribution, ledger, billing or trust-accounting code exists yet.
