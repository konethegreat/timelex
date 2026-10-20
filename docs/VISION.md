# Vision

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

## One sentence
An open, API-first practice-management platform for South African law firms in which **the routine work of recording, attributing, narrating and billing time is done by software, and people handle only what software cannot prove it got right.**

## The problem
- Fee earners lose billable time because recording it is a chore done after the fact. Vendors quote 30–50% recovered time, but those are vendor claims and unverified ([research 02](research/02-competitive-landscape.md)).
- The incumbent in South Africa is a desktop system with no public API, so a firm cannot build on it or move off it easily ([research 01](research/01-ghostpractice-teardown.md)).
- AI time-capture tools exist, but every one we found still expects a human to review each draft, and none is South-African-first.

## Who it is for
| Persona | Needs | TimeLex gives |
|---|---|---|
| **Fee earner** (attorney, candidate attorney) | Never type a time entry again; trust the narrative | Review only exceptions; see *why* each entry exists |
| **Practice manager / partner** | Realisation, WIP ageing, billing on time | Pre-bill that is already assembled; dashboards from real data |
| **Firm IT / systems admin** | Control, security, integration | Self-hostable services, BYO keys, audit log, SSO |
| **Agent / developer** | Programmatic access | REST, MCP tools, SDKs, a Claude plugin for every capability |
| **Bookkeeper / auditor** (Phase 7) | Compliant trust records | Immutable ledgers, reconciliations, audit pack |

## What "fully autonomous" means
Not "no humans". It means **autonomy is earned and measured**: each decision type has a confidence threshold, a policy, a sampling audit and a kill switch. Anything the system cannot prove, it escalates. Anything that moves money, signs, files or reaches a client is **never** autonomous by default. Details in [AUTONOMY.md](AUTONOMY.md) and [ADR-0005](adr/0005-autonomy-ladder-and-review.md).

## Product principles
1. **Capture is passive; attribution is evidence-based; posting is policy-governed.**
2. **Every output is traceable** to activity events (who, what, when, where it came from).
3. **Money is integers; rules are code with tests; compliance rules cite their source.**
4. **Small services, hard contracts.** Each capability is usable alone: API, MCP server, or plugin.
5. **The customer owns the keys and the data.** Self-hostable; BYO model and connector credentials; exportable at any time.
6. **Privacy by construction.** Capture metadata first; content only when needed; redact before any model call; local-first on the desktop.
7. **Measure before claiming.** No accuracy, savings or compliance claim ships without an evaluation behind it.

## What we deliberately will not do (yet, or ever)
- Give legal advice or draft documents for clients to sign without a qualified person's review (also an Anthropic usage-policy requirement; see [research 03](research/03-south-africa-compliance.md)).
- Move trust money autonomously. Trust accounting is regulated by the Legal Practice Council's accounting rules, is Phase 7, and is gated by an attorney and auditor review.
- Build a general accounting ledger. We integrate with the one the firm already uses, and ship a firm-specific ledger only where the LPC rules require it.
- Reverse-engineer or bypass GhostPractice's licence terms. Interop uses sanctioned exports and imports ([ADR-0009](adr/0009-ghostpractice-interop-strategy.md)).

## Success measures (proposed; calibrate on the first gold set)
| Measure | Target | How measured |
|---|---|---|
| Silent critical errors (wrong client billed, wrong amount) | 0 | Sampled audit + eval harness |
| Entries posted without human touch | ≥ 80% by end of pilot | Ledger decision log |
| Attribution precision at the auto-post threshold | ≥ 98% | Gold set + weekly sample |
| Fee-earner review time | ≤ 5 min/day median | Review-queue telemetry |
| Narratives with all numbers traceable | 100% | Number guard |
| Model spend per fee earner | ≤ R60/month (illustrative) | Per-tenant cost log ([research 06](research/06-strategy-and-positioning.md)) |

## Predecessors
Two earlier prototypes (a static HTML prototype and a Next.js prototype with a simulated sync gateway) proved the review workflow but not the hard parts: real capture, attribution accuracy, a real integration path or a real ledger. They are superseded by this repository; useful ideas (six-minute units, draft-review-approve, tenant-scoped drafts) are carried forward as requirements, not code.
