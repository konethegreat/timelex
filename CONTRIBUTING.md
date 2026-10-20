# Contributing

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Thanks for helping. TimeLex is a multi-language monorepo of small services ([architecture](docs/ARCHITECTURE.md)). It is pre-alpha, so the most valuable contributions right now are **research validation, review of the compliance assumptions, and contract feedback**.

## Setup
| Area | Tools | Check |
|---|---|---|
| Rust (`crates/`, `services/`) | stable Rust ≥ 1.88 | `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |
| Python (`workers/py-worker`) | Python ≥ 3.11, [uv](https://docs.astral.sh/uv/) | `uv sync && uv run ruff check . && uv run ruff format --check . && uv run pytest` |
| TypeScript (`packages/`) | Node 22 | `npm ci && npm run build && npm test` |
| Docs | Python 3 | `python3 tools/check_links.py` |

## Ground rules
1. **Contracts first.** Change `contracts/` before code; additive within a major version.
2. **Money is integer minor units (cents); durations are integer seconds.** No floats for either.
3. **Business rules live in Rust domain code**, never in Python workers or TypeScript clients.
4. **Every rule has a test**; compliance rules cite their source in a comment or doc.
5. **No real client data, email content, personal information or secrets** in code, tests, fixtures, issues or logs. Use synthetic data and the reserved `.test`/`.example` domains.
6. **Autonomy changes** (anything that posts, sends, moves money or deletes without a human) need an ADR update and a review against [docs/AUTONOMY.md](docs/AUTONOMY.md).
7. **Costs are in South African Rand** with the exchange-rate assumption stated; never USD as the main unit.
8. Use conventional commits (`feat:`, `fix:`, `docs:`, `chore:`). Small PRs. Link the task ID from [docs/TASKS.md](docs/TASKS.md).
9. Record notable work in [docs/WORKLOG.md](docs/WORKLOG.md); record decisions as ADRs in [docs/adr/](docs/adr/).
10. Source files carry an attribution header (for example `// Written by Kone Tshivhinda & Claude`); contributors may add their own line below it.

## Proposing a decision
Open a "Decision proposal" issue or a PR adding an ADR from [the template](docs/adr/0000-template.md).

## Licence
By contributing you agree your work is licensed under [Apache-2.0](LICENSE). Contributions are accepted under the Developer Certificate of Origin: sign off commits with `git commit -s`.
