<div align="center">

# TimeLex

**Autonomous practice management for law firms.** Captures the work, attributes it to the right matter, writes the narrative, prepares the bill, and asks a human only when it must.<br>Built as small services you can call over **REST**, **MCP** or a **Claude plugin**. Bring your own API keys; the model bill is yours.

[![CI](https://github.com/konethegreat/timelex/actions/workflows/ci.yml/badge.svg)](https://github.com/konethegreat/timelex/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)

</div>

> ⚠️ **Pre-alpha.** Today this repository contains the research, architecture decisions, phase plan and a walking skeleton (a tested billing-unit library, shared contract types, a gateway health endpoint, a Python redaction worker and a TypeScript SDK stub). **No capture, attribution, ledger, billing or trust-accounting code exists yet.** See the [roadmap](docs/PHASES.md). Not legal, tax or accounting advice.

## Why this exists

South African law firms run on desktop practice-management software built before cloud, APIs and AI. The market leader, GhostPractice, reports roughly 10 000 users, is described by reviewers as easy and well supported but also dated, with a closed financial module and no public API ([teardown](docs/research/01-ghostpractice-teardown.md)). Every AI time-capture product we found still expects a human to review each draft ([landscape](docs/research/02-competitive-landscape.md)). Nobody is shipping a **provable, policy-governed, near-autonomous** pipeline, and nobody in the South African market is shipping an **open, API-first** one.

## What TimeLex does (target)

| Stage | What happens | Human involved? |
|---|---|---|
| **Capture** | Email, calendar, Teams/Meet, documents and desktop activity become normalised *activity events* | No |
| **Attribute** | Events are grouped into time blocks and matched to a client and matter, with a confidence score and cited evidence | Only below the confidence threshold |
| **Narrate** | A fee-ready narrative is written; a number guard rejects any figure not in the evidence | No |
| **Post** | Entries are posted to the ledger under the firm's autonomy policy | Exceptions and sampled audits |
| **Bill** | WIP, pro-forma and fee notes are prepared (invoice maths, VAT and export through [ClearCut](https://github.com/konethegreat/clearcut)) | Fee earner certifies, if policy says so |
| **Run the practice** | Matters, conflicts, tasks, documents, then (last, and gated) trust accounting | By design, more |

"Fully autonomous" in TimeLex means **exception-based**: the system decides everything it can *prove* it got right, and routes the rest to a person. See [AUTONOMY.md](docs/AUTONOMY.md) for the ladder, the things that are never autonomous, and why.

## Design principles

1. **Microservices as contracts, not as sprawl.** Every capability has an OpenAPI contract, an MCP tool surface and its own data. We deploy few processes first ([ADR-0001](docs/adr/0001-service-boundaries-and-contract-first.md)).
2. **MCP host backend.** A server-side agent runtime connects to MCP servers (ours and third-party) and runs agent loops under policy, budgets and audit ([ADR-0003](docs/adr/0003-mcp-host-and-agent-runtime.md)).
3. **Bring your own keys.** Each firm stores its own Anthropic and connector credentials in an envelope-encrypted vault; we never carry their model cost ([ADR-0004](docs/adr/0004-byo-keys-and-secrets-vault.md)).
4. **Evidence or it did not happen.** Every entry, narrative and bill line cites the activity events behind it. Events are append-only; corrections supersede, never overwrite.
5. **Correct money.** Integer minor units (cents), deterministic rounding, no floats.
6. **Untrusted content stays untrusted.** Emails and documents are data, never instructions ([threat model](docs/THREAT_MODEL.md)).
7. **Right language for the job.** Rust for the core, gateway and agent host; Python for ML and OCR workers; TypeScript for the web app, Outlook add-in and SDK. Three is the ceiling ([ADR-0002](docs/adr/0002-language-allocation.md)).

## Architecture at a glance

```
 Outer agents (Claude, plugins)        Web app · Outlook add-in · SDKs        Desktop agent (Windows)
            │ MCP                              │ REST                                  │ HTTPS
            └───────────────┬──────────────────┴───────────────────────────────────────┘
                      ┌─────▼─────┐   authN/Z · tenancy · rate limits · routing
                      │  gateway  │
                      └─────┬─────┘
          ┌─────────────────┼───────────────────────────────────────────────┐
   ┌──────▼──────┐   ┌──────▼──────┐   ┌─────────────┐  ┌───────────┐  ┌────▼─────┐
   │  mcp-host   │◄─►│  vault (BYO │   │ capture-hub │─►│attribution│─►│ narrative│
   │ agent loops │   │ keys/tokens)│   │ connectors  │  │ (+py wkr) │  │  guarded │
   └──────┬──────┘   └─────────────┘   └─────────────┘  └───────────┘  └────┬─────┘
          │  MCP/REST to every service below                                  │
   ┌──────▼──────┐   ┌─────────────┐   ┌─────────────┐  ┌───────────┐  ┌────▼─────┐
   │ time-ledger │──►│   billing   │──►│  ClearCut   │  │  matters  │  │  trust   │
   │ autonomy    │   │ WIP · pro-  │   │ (invoices,  │  │ (clients, │  │(regulated│
   │ policy      │   │ forma · fee │   │ VAT, export)│  │ conflicts)│  │ Phase 7) │
   └─────────────┘   └─────────────┘   └─────────────┘  └───────────┘  └──────────┘
        Postgres (schema per service) · object storage · outbox → event bus
```

Full detail: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Repository layout

```
contracts/    OpenAPI, JSON Schema (events), MCP tool catalogue: the source of truth
crates/       Rust libraries: timelex-units (billing units), timelex-types (contract types)
services/     One folder per service; gateway is built, the rest are specified (README per service)
workers/      Python workers (redaction now; OCR/ML later). No business rules.
packages/     TypeScript SDK (web app and Outlook add-in arrive in Phase 4–5)
plugin/       Claude plugin manifest
eval/         Evaluation harness and gold-set spec (accuracy is measured, not asserted)
deploy/       Docker Compose dev stack (Phase 1)
docs/         Research, architecture, ADRs, phases, tasks, worklog
```

## Quick start

```bash
cargo test --workspace                   # Rust: units + contract types + gateway
cargo run -p timelex-gateway             # http://127.0.0.1:8088/health
(cd workers/py-worker && uv sync && uv run pytest)   # Python: redaction worker
npm ci && npm run build && npm test      # TypeScript SDK
```

```bash
# Billing-unit maths today (library): 47 minutes at R2 400/h, 6-minute units, round up
# → 8 units · R1 920.00     (see crates/timelex-units)
```

## Documentation

Start at [docs/INDEX.md](docs/INDEX.md). The short version:
[Vision](docs/VISION.md) · [Architecture](docs/ARCHITECTURE.md) · [Phases](docs/PHASES.md) · [Tasks](docs/TASKS.md) · [Autonomy](docs/AUTONOMY.md) · [Threat model](docs/THREAT_MODEL.md) · [ADRs](docs/adr/) · [Research](docs/research/)

## Relationship to ClearCut

[ClearCut](https://github.com/konethegreat/clearcut) reads, validates, narrates and exports invoices (Rust, Apache-2.0, REST + MCP). TimeLex produces the time and WIP that become invoices, and calls ClearCut for invoice maths, VAT and exports. They share conventions but no code; the contract is the API ([ADR-0008](docs/adr/0008-clearcut-relationship.md)).

## Contributing and security

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [GOVERNANCE.md](GOVERNANCE.md). Report vulnerabilities privately via [SECURITY.md](SECURITY.md). Questions go to [SUPPORT.md](SUPPORT.md).

## Author

Kone Tshivhinda · Written by Kone Tshivhinda & Claude · Licensed under [Apache-2.0](LICENSE).
