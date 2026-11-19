# Dependabot PRs (#51-#54) and CI Action Pin Audit

Date: 2026-11-19
Task: `t_fcc964c1` (Goal `G-048`, Phase 1)

## 1. Dependabot PRs #51-#54

| PR | State | Title | Scope / Risk | Recommendation |
| --- | --- | --- | --- | --- |
| #51 | CLOSED | `chore(deps): Bump chrislusf/seaweedfs from 4.51 to 4.52 in /deploy` | Docker Compose image bump (`deploy/docker-compose.yml`). All 12 checks passed; closed/superseded. | No action needed. |
| #52 | CLOSED | `chore(deps-dev): Bump ruff from 0.17.3 to 0.17.4 in /workers/py-worker` | Python dev linter bump. All 12 checks passed; closed/superseded by newer ruff update. | No action needed. |
| #53 | OPEN (MERGEABLE) | `chore(deps): Bump the actions group across 1 directory with 3 updates` | Updates `github/codeql-action/{init,analyze,upload-sarif}` from `9b3254085494b55a0f7c4c2b3d2d52cc712ce370` (`v4.39.1`) to `33d7b85da1a04171e211befe2c739303b9a4c066` (`v4.39.3`) across `.github/workflows/codeql.yml` and `.github/workflows/scorecard.yml`. All 12 checks pass. | Low risk; safe to merge. |
| #54 | OPEN (MERGEABLE) | `chore(deps): Bump the rust-minor-patch group across 1 directory with 2 updates` | Bumps `rmcp` exact pin (`=3.8.0` to `=3.10.0`) in `crates/timelex-mcp/Cargo.toml` and updates `Cargo.lock` (`rmcp` 3.10.0, `rmcp-macros` 3.11.0, `uuid` 1.29.2). `timelex-mcp` uses an exact pin on purpose so transport/error changes are gated by contract tests; all 12 CI checks including `Rust (fmt, clippy, tests, contracts)` pass. | Low-to-medium risk; safe to merge after standard review. |

## 2. CI Workflow Action Pin Consistency (`.github/workflows/`)

Audited all 5 workflow files: `ci.yml`, `codeql.yml`, `dependency-review.yml`, `labels.yml`, and `scorecard.yml`.

- **SHA pinning**: Every `uses:` reference across all 5 workflows is pinned to a 40-character commit SHA with an inline `# vX.Y.Z` version comment.
- **Cross-workflow consistency**:
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1` is identical across all 9 occurrences in all 5 workflows.
  - `astral-sh/setup-uv@68559e99cc807c1eb7e9238fd43a15bba460eec2 # v10.3.3` is identical across `python` and `eval` jobs in `ci.yml`.
  - `github/codeql-action/{init,analyze,upload-sarif}@9b3254085494b55a0f7c4c2b3d2d52cc712ce370 # v4.39.1` is identical across `codeql.yml` and `scorecard.yml` (and PR #53 updates all 3 together to `v4.39.3`).
  - Single-use actions (`dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `actions/setup-node`, `actions/dependency-review-action`, `crazy-max/ghaction-github-labeler`, `ossf/scorecard-action`, `actions/upload-artifact`) are all SHA-pinned with version comments.
- **Permissions**: Least-privilege `permissions:` blocks are declared across all workflows (`contents: read` baseline, with write scopes restricted to specific jobs requiring `security-events: write`, `id-token: write`, or `issues: write`).
- **Dependabot coverage**: `.github/dependabot.yml` configures weekly updates for `cargo`, `npm`, `uv` (`/workers/py-worker`), `github-actions` (grouped), and `docker-compose` (`/deploy`).
