# Documentation index

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| Read this | When |
|---|---|
| [VISION.md](VISION.md) | What we are building, for whom, and what we refuse to build |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Services, contracts, data, tenancy, deployment |
| [AUTONOMY.md](AUTONOMY.md) | What "fully autonomous" means here, and its limits |
| [PHASES.md](PHASES.md) | Roadmap, exit criteria and gates |
| [TASKS.md](TASKS.md) | The backlog: task IDs, owners, dependencies, acceptance |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Assets, trust boundaries, threats and mitigations |
| [GLOSSARY.md](GLOSSARY.md) | Terms used across docs |
| [WORKLOG.md](WORKLOG.md) | Running log of work and decisions in flight |

## Research (read in order)

| # | File | Question it answers |
|---|---|---|
| 01 | [ghostpractice-teardown](research/01-ghostpractice-teardown.md) | What is GhostPractice, what does it do, where is it weak? |
| 02 | [competitive-landscape](research/02-competitive-landscape.md) | Who else captures time automatically, and how autonomous are they? |
| 03 | [south-africa-compliance](research/03-south-africa-compliance.md) | LPC accounting rules, POPIA, Anthropic usage policy: what constrains us? |
| 04 | [autonomous-capture-design](research/04-autonomous-capture-design.md) | How do we capture, attribute and narrate with provable accuracy? |
| 05 | [mcp-host-and-byo-keys](research/05-mcp-host-and-byo-keys.md) | MCP 2026-07-28, host design, bring-your-own-key economics |
| 06 | [strategy-and-positioning](research/06-strategy-and-positioning.md) | Wedge, scope, pricing in Rand, risks, what to validate first |
| 07 | [anthropic-terms-and-key-custody](research/07-anthropic-terms-and-key-custody.md) | Retention, ZDR, residency, DPA, usage policy; key-custody options and questions for Anthropic |
| 08 | [mcp-rust-sdk-spike](research/08-mcp-rust-sdk-spike.md) | Does the Rust MCP SDK serve the 2026-07-28 spec? Spike results and settings |

## Validation kits

| Read this | When |
|---|---|
| [interview-kit](validation/interview-kit.md) | Running T0-R01 and T0-B01 interviews: hypotheses, scripts, Rand questions, synthesis |
| [demo-checklist](validation/demo-checklist.md) | One page for the GhostPractice demo and the TimeLex storyboard session |

## Architecture decision records

| ADR | Title | Status |
|---|---|---|
| [0001](adr/0001-service-boundaries-and-contract-first.md) | Service boundaries and contract-first microservices | Accepted |
| [0002](adr/0002-language-allocation.md) | Language allocation: Rust, Python, TypeScript | Accepted |
| [0003](adr/0003-mcp-host-and-agent-runtime.md) | MCP host and agent runtime | Proposed |
| [0004](adr/0004-byo-keys-and-secrets-vault.md) | Bring-your-own keys and the secrets vault | Proposed |
| [0005](adr/0005-autonomy-ladder-and-review.md) | Autonomy ladder and exception-based review | Accepted |
| [0006](adr/0006-event-sourced-activity-and-ledger.md) | Event-sourced activity and an append-only ledger | Accepted |
| [0007](adr/0007-tenancy-and-data-residency.md) | Multi-tenancy and data residency | Proposed |
| [0008](adr/0008-clearcut-relationship.md) | Relationship to ClearCut | Accepted |
| [0009](adr/0009-ghostpractice-interop-strategy.md) | GhostPractice interoperability and migration | Proposed |
| [0010](adr/0010-messaging-and-sync-contracts.md) | Messaging and synchronous contracts | Proposed |
| [0011](adr/0011-tenancy-enforcement-and-database-roles.md) | Tenancy enforcement and database roles | Accepted |
| [0012](adr/0012-internal-service-authentication.md) | Internal service authentication and gateway routing | Proposed |

Use [adr/0000-template.md](adr/0000-template.md) for new decisions.
