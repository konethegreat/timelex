## What and why

Link the task ID from `docs/TASKS.md` (for example `P1-03`) and the ADR if this changes a decision.

## How it was tested

## Checklist
- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (if Rust changed)
- [ ] `uv run ruff check . && uv run pytest` pass in `workers/py-worker` (if Python changed)
- [ ] `npm run build && npm test` pass (if TypeScript changed)
- [ ] Money is integer minor units; durations are integer seconds (no floats)
- [ ] Contract changes are in `contracts/` and backward compatible within the major version
- [ ] No secrets, client data or real personal information in code, tests, examples or logs
- [ ] Docs, `docs/TASKS.md` statuses and `docs/WORKLOG.md` updated where behaviour or plans changed
- [ ] Anything that could affect autonomy, trust money or client-facing output was reviewed against `docs/AUTONOMY.md`
