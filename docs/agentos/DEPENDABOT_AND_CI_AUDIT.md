# Dependabot PRs #51-#54 and CI Workflow Action Pin Audit

Date: 2026-11-18
Task: `t_af72c4dc` (Phase 1: Research, Hygiene, and Stale-Doc Alignment)

## 1. Dependabot PRs #51 to #54

| PR | Title | State | Target Files | Assessment & Risk | Recommendation |
|---|---|---|---|---|---|
| [#51](https://github.com/konethegreat/timelex/pull/51) | `chore(deps): Bump chrislusf/seaweedfs from 4.51 to 4.52 in /deploy` | CLOSED (2026-11-17) | `deploy/docker-compose.yml` | Low risk dev-stack S3 image bump; all 12 CI checks passed before closure. Superseded by open PR [#58](https://github.com/konethegreat/timelex/pull/58) (`4.51` to `4.53`, all 12 CI checks passing). | Keep closed; review and merge superseding PR #58. |
| [#52](https://github.com/konethegreat/timelex/pull/52) | `chore(deps-dev): Bump ruff from 0.17.3 to 0.17.4 in /workers/py-worker` | CLOSED (2026-11-17) | `workers/py-worker/uv.lock` | Low risk dev-only linter/formatter lockfile update; all 12 CI checks passed before closure. Superseded by open PR [#59](https://github.com/konethegreat/timelex/pull/59) (`0.17.3` to `0.17.5`, all 12 CI checks passing). | Keep closed; review and merge superseding PR #59. |
| [#53](https://github.com/konethegreat/timelex/pull/53) | `chore(deps): Bump the actions group across 1 directory with 3 updates` | OPEN | `.github/workflows/codeql.yml`, `.github/workflows/scorecard.yml` | Very low risk patch bump of `github/codeql-action/{init,analyze,upload-sarif}` from `v4.39.1` (`9b3254085494b55a0f7c4c2b3d2d52cc712ce370`) to `v4.39.3` (`33d7b85da1a04171e211befe2c739303b9a4c066`). Preserves full 40-char SHA pinning and cross-workflow consistency; all 12 CI checks pass. | Safe to merge. |
| [#54](https://github.com/konethegreat/timelex/pull/54) | `chore(deps): Bump the rust-minor-patch group across 1 directory with 2 updates` | OPEN | `crates/timelex-mcp/Cargo.toml`, `Cargo.lock` | Low-to-moderate risk. Bumps exact pin `rmcp` `=3.8.0` to `=3.10.0` (with `rmcp-macros` `3.11.0` and `sha2 0.11.1`) and `uuid` `1.29.0` to `1.29.2`. `crates/timelex-mcp/Cargo.toml` intentionally uses an exact `=` pin per [research 08](../research/08-mcp-rust-sdk-spike.md) so transport/error changes must pass contract tests. All 12 CI checks pass, including `Rust (fmt, clippy, tests, contracts)`. | Safe to merge (exact pin preserved and contract tests pass). |

## 2. CI Workflow Action Pin Consistency

Inspected all 5 workflow files under `.github/workflows/` (`ci.yml`, `codeql.yml`, `dependency-review.yml`, `labels.yml`, `scorecard.yml`) and `.github/dependabot.yml`:

- Every `uses:` step across all 5 workflows is pinned to a full 40-character commit SHA with a matching version comment.
- Shared actions use identical SHAs and versions across all workflows:
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1` (`v7.0.1`): 9 uses across all 5 workflows (`ci.yml` x5, `codeql.yml` x1, `dependency-review.yml` x1, `labels.yml` x1, `scorecard.yml` x1).
  - `github/codeql-action/{init,analyze,upload-sarif}@9b3254085494b55a0f7c4c2b3d2d52cc712ce370` (`v4.39.1`): 3 uses across `codeql.yml` and `scorecard.yml`.
  - `astral-sh/setup-uv@68559e99cc807c1eb7e9238fd43a15bba460eec2` (`v10.3.3`): 2 uses in `ci.yml`.
- Single-use actions are also SHA-pinned: `dtolnay/rust-toolchain` (`v1`), `Swatinem/rust-cache` (`v2.9.3`), `actions/setup-node` (`v7.1.0`), `actions/dependency-review-action` (`v5.0.0`), `crazy-max/ghaction-github-labeler` (`v6.0.0`), `ossf/scorecard-action` (`v2.5.0`), and `actions/upload-artifact` (`v7.0.2`).
- `.github/dependabot.yml` groups `github-actions` updates (`patterns: ["*"]`), keeping future action updates synchronized across workflows (as demonstrated by PR #53).

Result: No workflow pin inconsistencies found; no workflow changes required.
