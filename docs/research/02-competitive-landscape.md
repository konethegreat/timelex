# 02 · Competitive landscape

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

**Source warning.** Most comparison articles are written by vendors in the category (Ajax, PointOne and LegalTime AI each rank themselves well). We use them for *what exists* and *how it works*, never for *which is best*. No source gave an independent accuracy figure. Prices are as published in those articles and converted at an assumed **R18/USD** (the rate used in ClearCut's ADR-0005); confirm the live rate before using any figure externally.

## 1. The category in one line
Every product in this category **drafts** time and expects a human to **review and release** it. One article states it flatly: "Every tool in this category still expects a human to review and adjust the draft narrative" ([LegalAIWorld](https://legalaiworld.com/best-ai-billing-time-tracking-tools-for-law-firms-2026/)). Another: "Automatic is not unsupervised billing" ([LegalTime AI](https://www.legaltime.ai/blog/best-automatic-time-tracking-software-for-lawyers)). **The "autonomous" position is unoccupied. It is unoccupied partly because it is risky**, which is why [AUTONOMY.md](../AUTONOMY.md) exists.

## 2. Passive / AI capture products

| Product | How it captures | Review model | Published price (USD → ZAR at R18) | Notes and limits |
|---|---|---|---|---|
| **Ajax** | Desktop app reads on-screen content across apps | Drafts for review | ~$47–169/user/mo → **R846–R3 042** | Per-attorney desktop install; premium; integrates with Clio, MyCase, PracticePanther, SurePoint, Filevine, others |
| **PointOne** | Passive capture: email, documents, calls, web; says no keystrokes or screen content | Drafts matched to firm policy and client billing guidelines | Custom | Mid-to-large firms; billing-guideline enforcement |
| **Laurel** (ex-Time by Ping) | Passive, content-level capture | Not specified | Custom, enterprise | Recently skewed to accounting firms |
| **Clio Manage / Duo / "Manage AI"** | Suggestions from data already in Clio; AI improves manual entries | Suggestions, not passive | Clio $49–159 → **R882–R2 862**; AI add-on ~$49–59 → **R882–R1 062** | Only sees work done inside Clio; AI gated to higher tiers |
| **Smokeball AutoTime** | Passively tracks Microsoft 365 apps | Drafts | $49 / $89 → **R882 / R1 602** (higher tiers custom) | Misses non-Microsoft work; Mac support lagged |
| **Rocket Matter Track** | Built-in AI passive capture, voice entry | Drafts | $39 / $89 → **R702 / R1 602** | AI accuracy "varies" |
| **MyCase Smart Time Finder** | Reviews calendar, email, activity *after the fact* | Drafts | From $39 → **R702** | Not real-time |
| **Billables.ai** | API metadata from Microsoft 365, Google, Zoom; AI narratives | Drafts | $39–99 → **R702–R1 782** | Shallower than screen reading; young company |
| **LegalTime AI** | Local capture on the attorney's machine; pause control | Drafts visible only to the attorney, released on approval | ~$125 → **R2 250** (G2 directional) | Not SOC 2 certified (Type II in progress, per its own article) |
| **BigHand SmartTime / Intapp Time** | Active timers plus passive monitoring | Drafts | Enterprise | Large firms; heavy deployments |
| **Lawgro MagicTime** | Background capture from email, Word, court sites, research tools | User reviews and files | Not stated | Small firms; compliance rated limited |
| **Tempello** | Email only, matter matching by metadata and AI | Creates entries in Clio / MyCase | Per processed email | Supplement, not a suite |
| **WiseTime** | Autonomous background tagging of activity | Attorneys finalise | $12–40/user/mo → **R216–R720** | "Activity tracker" per reviewers |
| **Memtime** | Local-only timeline | User builds entries | $12–40 → **R216–R720** | A memory aid, no matter assignment or narrative |

## 3. Manual-entry practice systems (no automatic capture)
TimeSolv ($35–50), PracticePanther ($49/$89), CosmoLex ($89–99), Bill4Time (~$39) per the [Ajax article](https://joinajax.com/blog/legal-time-tracking-software); Rand: roughly R630–R1 800. These show what firms pay even with no AI.

## 4. South African practice systems
Directory listings show several local or locally-used products (LegalSuite, Legal Interact "Practice Manager", LawPracticeZA, Legal Unity and others) alongside Clio and GhostPractice ([Capterra ZA](https://www.capterra.co.za/alternatives/213777/ghostpractice), [Stanford TechIndex](https://techindex.law.stanford.edu/companies/16511)). **We have not torn these down.** Task **T0-R02** does that. Until then we make no claim about local competitors other than GhostPractice.

## 5. Open source
A search for open-source legal practice management found only generic case-management and Odoo-module offerings, nothing comparable to a South African, AI-native, MCP-capable system ([Worklenz roundup](https://www.worklenz.com/blog/05-best-open-source-case-management-software-for-your-law-firm), [Odoo apps](https://apps.odoo.com/apps/modules/17.0/law_firm_case_management)). Treat as "no strong incumbent found", not as proof none exists.

## 6. Gap analysis

| Dimension | Best of market today | Gap TimeLex targets |
|---|---|---|
| Capture breadth | Screen reading (Ajax), M365 only (Smokeball), metadata only (Billables) | Hybrid: API connectors for mail, calendar, meetings plus a **local-first** desktop agent |
| Autonomy | Draft, then human review of each draft | **Exception-based review** with measured thresholds and sampling audits |
| Evidence | Mostly opaque | Every entry cites its activity events; narratives pass a number guard |
| Openness | Closed, "contact sales" | Apache-2.0, self-hostable, REST + MCP + plugin, transparent price |
| Cost model | Per-seat licence bundles AI | **BYO API keys**: the firm pays model cost directly; platform fee covers software only |
| Local fit | US-centric (Clio, Smokeball, Ajax) | SA VAT, LPC rules, POPIA, Rand, SA matter terminology |
| Integration | Few have an API; GhostPractice has none | API-first; import/export paths for GhostPractice in shadow mode |
| Governance | Rarely described | Autonomy policy per tenant/matter, kill switch, audit, undo |

## 7. Positioning statement
For South African law firms that lose time to manual recording, TimeLex is an **open, API-first practice platform whose time capture is exception-based rather than review-everything**, with evidence behind every entry and the firm's own API keys paying for the AI. Unlike GhostPractice it has an API and no desktop lock-in; unlike Ajax, PointOne or Clio it is open, local, and honest about where autonomy stops.

## 8. What this does *not* prove
- That firms want less review. Some partners may insist on seeing everything. The autonomy ladder lets them.
- That our accuracy will beat vendors'. Nobody published independent numbers, including us. Phase 3's evaluation harness is how we find out.
- That price is the lever. At R700–R3 000 per user per month for incumbents versus an estimated model spend of tens of Rand ([research 06](06-strategy-and-positioning.md)), there is room, but willingness to switch depends on trust, not price.

## Sources
[Timing: lawyer time tracking](https://timingapp.com/blog/lawyer-time-tracking-software/) · [PointOne: AI timekeeping](https://pointone.com/blog/best-ai-timekeeping-software) · [Ajax: legal time tracking](https://joinajax.com/blog/legal-time-tracking-software) · [LegalTime AI: automatic tracking](https://www.legaltime.ai/blog/best-automatic-time-tracking-software-for-lawyers) · [LegalAIWorld](https://legalaiworld.com/best-ai-billing-time-tracking-tools-for-law-firms-2026/) · [Capterra ZA alternatives](https://www.capterra.co.za/alternatives/213777/ghostpractice)
