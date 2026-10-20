# 06 · Strategy and positioning

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

This file turns research 01–05 into decisions and flags where we are pushing back on the brief. Pushback is deliberate: the goal is a second opinion, not agreement.

## 1. The brief, restated
Replace GhostPractice with a **fully autonomous**, **multi-language**, **microservice**, **MCP-host** backend where **customers bring their own API keys**, connected to ClearCut.

## 2. Scope: "replace GhostPractice" is three products
| Tier | Contents | Regulated? | Realistic order |
|---|---|---|---|
| **A. Autonomous time and billing** | Capture, attribution, narrative, ledger of time, WIP, pro-forma, fee-note prep (via ClearCut), exports | No (fee accounts only, rule 54.12) | **First.** This is the wedge and the only tier where "autonomous" is differentiating today |
| **B. Practice core** | Clients, matters, conflicts, contacts, tasks, key dates, court calendar, documents, assembly, reporting, FICA | Partly (FICA, POPIA) | Second |
| **C. Regulated money** | Business and trust ledgers, trust receipts and payments, reconciliations, LPC reports, audit pack | **Yes** (Part XII, rules 54–57) | **Last, gated** by attorney and auditor review |

**Pushback #1: do not start with a full replacement.** GhostPractice has about 10 000 users, high satisfaction scores, deep onboarding and a regulated money core. A big-bang replacement has to match everything on day one. A wedge that runs *beside* GhostPractice (shadow mode) can win fee earners first and earn the right to replace the back office later. Tier C can be the last thing we build and the first thing we let a partner (an accounting-software vendor) provide, if that proves safer.

## 3. Autonomy
**Pushback #2: "minimal human input" should be a measured outcome, not a switch.** Competitors with far more capital still ship review-every-draft, and the written position of the category is that automatic is not unsupervised billing. We lead on **exception-based review with evidence and sampling**, which is genuinely different and defensible. Fee notes, trust movements and legal documents keep a human gate by default ([AUTONOMY.md](../AUTONOMY.md)). POPIA section 71 and Anthropic's legal high-risk rules both point the same way ([research 03](03-south-africa-compliance.md)).

## 4. Microservices
**Pushback #3: microservices are an organisation-scaling tool; you are one part-time developer.** The failure mode is a *distributed monolith*: many services, shared database, lock-step releases, and a debugging experience across ten logs. We keep your goal (everything usable over API, MCP and plugin; independently deployable) with guardrails ([ADR-0001](../adr/0001-service-boundaries-and-contract-first.md)):
- A boundary exists only if it has its own data, a versioned contract and a reason (scaling, security, language, or independent usefulness).
- Contract first: OpenAPI, JSON Schema, MCP catalogue live in `contracts/`.
- Service budget: **≤ 4 deployables through Phase 2, ≤ 8 by Phase 5.** Specified services without code are folders with a README, not running processes.
- One platform kit (auth, tenancy, telemetry, outbox) so each new service costs hours, not days.
- Schema-per-service in one Postgres cluster at first. No cross-schema reads.

## 5. Languages
**Pushback #4: "the fastest languages" is not the selection criterion.** Capture, attribution and narration are I/O- and model-latency-bound; Rust's speed barely moves them. Rust earns its place on **correctness** (money, state machines, policy), on the **desktop agent** (small, safe native binary) and on **the gateway/host** (predictable tail latency, memory safety for key handling). Python wins on OCR/ML libraries. TypeScript wins on the Outlook add-in (Office.js) and the web app. **Three languages is the ceiling**; Go and C# were considered and rejected for the same reason ClearCut rejected Go ([ADR-0002](../adr/0002-language-allocation.md)).

## 6. Differentiators worth building (ranked)
| # | Differentiator | Evidence it is open ground | Risk |
|---|---|---|---|
| 1 | Exception-based autonomy with measured thresholds | Category is review-everything | Accuracy must be proven (Phase 3 gate) |
| 2 | Evidence on every entry (citations to activity events) | Competitors opaque | Needs excellent UX |
| 3 | Open, API-first, MCP-native, plugin | GhostPractice has no API; others "contact sales" | Open source needs a business model (§8) |
| 4 | BYO keys, transparent platform price | Bundled AI in per-seat tiers | Support burden |
| 5 | South African fit (Rand, VAT, LPC, POPIA, terminology) | US-centric competitors | Small market, so keep costs low |
| 6 | Shadow mode beside GhostPractice | Switching cost is the main barrier | Needs a sanctioned export/import path (ADR-0009) |

## 7. Pricing sketch (Rand; hypothesis to test with five firms)
Competitor licences: **R700–R3 000** per user per month ([research 02](02-competitive-landscape.md)). Illustrative model spend paid by the firm: **R13–R56** per fee earner per month ([research 04](04-autonomous-capture-design.md)).

| Plan | Idea | Hypothesis |
|---|---|---|
| **Self-host (open source)** | Free; firm runs everything and brings keys | Community and credibility |
| **Hosted Team** | Per fee earner per month, platform only, BYO keys | **R250–R450** (to validate; well below competitors because we carry no AI cost) |
| **Hosted Firm + support** | Onboarding, shadow-mode setup, SLA | Fixed onboarding fee plus per-user |
| **Compliance add-on (Tier C)** | Only after review | Separate pricing |

These are guesses. Task **T0-B01** interviews five firms about willingness to pay.

## 8. Business model and licence
Apache-2.0 for the core (patent grant, permissive, matches ClearCut). Revenue from hosting, onboarding, support, SLAs and later the regulated modules. Risk: a cloud provider hosting it for free. Mitigation: brand, speed, support, trademark policy. Revisit if that materialises (a source-available licence is a one-way door and hard to undo). The name "TimeLex" has no obvious legal-software collision in a quick search, but a trademark search is a to-do (T0-B02).

## 9. Risks (top ten)
| # | Risk | Likelihood | Impact | Response |
|---|---|---|---|---|
| 1 | Attribution accuracy too low for autonomy | Medium | High | Hard keys first; calibrate; sample; fall back down the ladder |
| 2 | No sanctioned GhostPractice export/import | Medium | High | Shadow mode via reports/CSV; Office integration; partner conversation (ADR-0009) |
| 3 | Privilege/confidentiality objections to cloud LLMs | High | High | Local-first capture; redaction; in-region/self-hosted provider option; firm-controlled exclusions |
| 4 | Scope explosion (full practice suite) | High | High | Tier A first; phase gates; service budget |
| 5 | One part-time developer | High | High | Reuse ClearCut conventions; platform kit; small PRs; AI-assisted delivery with tests |
| 6 | Prompt injection via captured content | Medium | High | Threat model; no tools in extraction; policy outside the model |
| 7 | Regulatory change (POPIA guidance, LPC rules, e-invoicing) | Medium | Medium | Track in research files; compliance as tested code |
| 8 | Customer key misuse / runaway spend | Medium | Medium | Hard caps; alerts; recommended Anthropic workspace limits |
| 9 | Vendor claims of 30–50% recovery set unrealistic expectations | Medium | Medium | Publish our own measured numbers only |
| 10 | Dye & Durham ships AI into GhostPractice | Medium | Medium | Move fast on the wedge; be API-first and open |

## 10. What to validate first (before writing much more code)
1. **Demo/trial of GhostPractice and three user interviews** (T0-R01).
2. **Five fee-earner interviews:** how time is recorded today, what they would trust a machine to post (T0-B01).
3. **A synthetic-plus-consented gold set** for attribution (Gate A, tasks P3-01 and P3-02).
4. **Written answers from Anthropic** on retention and key-custody patterns (T0-L02).
5. **A lawyer's review** of research 03 (T0-L01).

If 1–2 show that fee earners already have an acceptable workflow, or that firms will not consider machine-posted time under any policy, reconsider the wedge before building Phase 2.
