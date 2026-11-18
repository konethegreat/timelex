# Report

Every pull request appends one dated summary line below.

- 2026-11-17: Initialized AgentOS planning docs (`docs/agentos/PHASES.md`, `docs/agentos/TASKS.md`, `docs/agentos/REPORT.md`) from repository research.
- 2026-11-18: README status banner and repository layout aligned with P3-01 to P3-07: banner names `matters`, `timelex-segmentation`, `timelex-attribution`, py-worker retrieval and `services/attribution` and states what is not built yet; layout lists every crate, separates services built and run in Compose (gateway, identity, vault, capture-hub, matters) from library-only (mcp-host, attribution) and specified-only ones. Verified against `docs/TASKS.md`, `Cargo.toml` files, `src/main.rs` presence and `deploy/docker-compose.yml`; added lines scanned for em/en dashes and placeholder markers. Documentation only, no code changed; cargo, npm and link checks run in CI (local runs were blocked by the session's permission mode).
