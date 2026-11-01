# ADR-0017: A generator for new services

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0001](0001-service-boundaries-and-contract-first.md), [ADR-0011](0011-tenancy-enforcement-and-database-roles.md), [ADR-0012](0012-internal-service-authentication.md) and [ADR-0016](0016-telemetry-kit.md); implements P1-11.

## Context

Identity, vault and gateway each carry the same operational frame: an internal-token check, a tenant transaction, a `/health` endpoint, problem-detail errors and telemetry. The frame is small but easy to get wrong, and the three copies already differ in details. Phase 2 adds capture-hub and more, and the first thing a new service must not do is invent a fourth authentication extractor or forget the row-level-security grant.

P1-11's acceptance is that a new service builds with authentication, tenancy, health and CI. The workspace also has conventions the code alone does not express: the crate must be a workspace member, the role and schema must exist in the Postgres bootstrap, and the database tests need the same throwaway-cluster harness.

## Decision

1. **A workspace tool, not an external scaffolder.** [`tools/new-service`](../../tools/new-service) is a Rust binary in the workspace with no dependencies. Templates are embedded with `include_str!`, so the tool ships with the repository and cannot fetch a template from a branch that no longer exists. `cargo-generate`, Yeoman and similar tools were rejected: they add a second toolchain and a network step for nine small files.

2. **One name rule, checked before anything is written.** `validate_name` accepts 2 to 31 characters: a lowercase letter, then lowercase letters, digits or underscores, and refuses `platform`, `public` and `pg_%`. That is the rule `platform.provision_service` enforces; the generator checks it again so a bad name fails with a sentence instead of a Postgres error halfway through. The tool refuses an existing `services/<name>` directory: a generator that merges with existing work is one bad edit away from destroying it.

3. **The generated service is complete and runnable, with one honest example.** It contains the gateway-token extractor with the three caller kinds, tenant transactions, `/health`, telemetry, one tenant-owned `items` table under the standard RLS policy with explicit grants, database-backed tests (health, 401 and 403, create and list, cross-firm isolation, bad input) and its own README. The `items` operations are labelled as an example in the code and the README; they exist to prove the frame end to end, not to be a product feature.

4. **Exactly two existing files are patched.** The root `Cargo.toml` `members` list and `deploy/postgres/bootstrap/004_provision_services.sql`. Both edits are idempotent. Contracts, gateway routes and compose services are deliberately not generated: each is a decision that needs a person and a review, and guessing them produces files that look done and are not.

5. **The templates are tested by generating a real member in CI.** The Rust job runs the generator for `example`, then checks formatting, clippy and the generated database tests, then restores the two patched files and removes the directory. A template that stops compiling or that drops the tenancy test fails the build. The generator's own unit tests generate into a temporary workspace, so they never touch the repository.

## Alternatives considered

- **A boilerplate repository or template branch.** It drifts silently: nothing fails when the template falls behind the workspace's lint settings or dependency versions. Generating inside the workspace in CI makes drift a test failure.
- **Copy `services/identity` by hand each time.** That is the status quo the task exists to end. The copies already differ, and the differences are exactly in the parts that matter.
- **Generating the contract, gateway route and compose service too.** Those span three subsystems and need names, scopes, paths and decisions the tool cannot infer. A wrong guess would be harder to find than a missing file. Left manual and listed as next steps in the generated README.
- **A generator for domain crates as well.** The vault and identity domain crates are where a service's rules live, and they are small and specific. There is no useful common skeleton beyond `Cargo.toml` settings, and the project deliberately keeps rule code simple (ADR-0002).
- **`regex` for the name rule.** One hand-written check is fewer dependencies and states the rule in the same terms as the SQL procedure.

## Consequences

- **A fourth copy of the authentication and problem-detail code exists.** That is intentional for now: the generated copy is the starting point a new service owns, and it can differ when its contract needs it. When the frame stops changing, moving it into a shared crate is a smaller change than parameterising a generator around every difference.
- **The name rule lives in two places.** `validate_name` and `platform.provision_service` must agree; the generated README and the tool's tests state both, and a rule change is a two-line change. A test against the SQL text was rejected as brittle.
- **CI gets slower.** The scratch service compiles against the workspace graph and runs its database tests. It is the only check that proves the templates still work, and it runs after the normal suite so a template failure is clearly attributed.
- **The default listen address is `127.0.0.1:8083`.** Identity uses 8081 and vault 8082; a second generated service needs a different port. The README says so and the variable `TIMELEX_<NAME>_ADDR` makes it a configuration change.
- **Generated code is formatted by the template, not by the tool.** The CI check runs `cargo fmt --check` on the generated member. If a future template line is near the width limit, CI fails and the template is fixed.
