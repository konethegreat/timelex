# ADR-0001: Service boundaries and contract-first microservices

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude

## Context
The brief is to avoid a monolith and make every aspect usable as an API, a plugin and an MCP server. The team is one part-time developer (plus AI assistance). The two earlier prototypes were single applications. The risk with many small services and a small team is a **distributed monolith**: shared database, lock-step releases, costly debugging ([research 06 §4](../research/06-strategy-and-positioning.md)).

## Decision
1. **A service exists only if** it (a) owns its data, (b) has a versioned public contract, and (c) has at least one of: a different scaling profile, a different security boundary, a different language need, or independent usefulness as an API/MCP/plugin.
2. **Contract first.** `contracts/` holds OpenAPI (REST), JSON Schema (events and payloads) and an MCP tool catalogue. Code is checked against contracts in CI; contracts are not generated after the fact.
3. **Every service exposes the same capability three ways** from one codebase: REST under `/v1`, MCP under `/mcp` (Streamable HTTP, stateless per the 2026-07-28 spec), and a typed SDK method. The MCP listener may be bound to a separate port so it can be firewalled differently. External agents reach MCP only through the gateway.
4. **Data ownership.** One Postgres cluster at first, **one schema and one database role per service**; no cross-schema reads or foreign keys. Cross-service facts travel by API or by events.
5. **Service budget.** At most **4 deployables through Phase 2** and **8 by Phase 5**. A "service" with no code yet is a folder with a README and a contract, not a running process.
6. **Platform kit.** Shared Rust crates (`timelex-types`, `timelex-units`, later `-telemetry`, `-authz`, `-outbox`) and a `tools/new-service` generator so a new service starts with auth, tenancy, health, tracing and CI.
7. **No shared code for business rules across services**; shared crates hold types and pure utilities only.
8. **Independent versioning** per contract (semver in the OpenAPI `info.version` and event `schemaVersion`); breaking changes need a new major path (`/v2`) and a deprecation window.

## Alternatives considered
- *Modular monolith first.* Easiest to build and the usual advice for a team of one. Rejected as the *default* because the brief and the market positioning ("every capability as an API/MCP") depend on hard boundaries from day one; adopted as a **fallback**: if the platform kit is not paying for itself by the end of Phase 2, merge services behind the same contracts. Because contracts are the boundary, merging is cheap.
- *Serverless functions per capability.* Poor fit for long-lived webhook subscriptions, renewals and a desktop-agent sync.
- *Shared database with a service layer.* Fastest, but it is the distributed-monolith failure mode.

## Consequences
- More up-front work (contracts, platform kit, local dev stack) before features.
- Better extensibility, ability to open-source services individually, and clean MCP/plugin surfaces.
- Local development uses Docker Compose ([deploy/](../../deploy/)); distributed tracing from the start.
- **Revisit** at the end of Phase 2 and Phase 5 using three signals: time to add a service, cross-service incident count, and the share of changes that touch more than one service.
