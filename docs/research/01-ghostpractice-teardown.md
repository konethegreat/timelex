# 01 · GhostPractice teardown

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

**Confidence:** medium-low. Everything below comes from third-party listings, a vendor marketing page and press releases. We have not seen the product, its documentation or a demo. Task **T0-R01** (get a demo or trial and interview three GhostPractice firms) is the first validation step, and the numbers in this file must be treated as hypotheses until then.

## 1. Who they are
- Developed by **Korbitec** in Cape Town; described as South Africa's largest legal practice-management provider before acquisition ([Legal Technology Hub](https://www.legaltechnologyhub.com/vendors/ghostpractice-by-dye-and-durham/)).
- **Acquired by Dye & Durham (TSX: DND), announced 23 May 2023; terms undisclosed.** The press release gives about 10 000 users, about 110 employees, mostly small and medium firms, and a presence in Canada ([Dye & Durham](https://dyedurham.com/dye-durham-acquires-practice-management-software-provider-ghostpractice)).
- Implication: the owner is a Canadian consolidator, not a South African product company. Consolidators typically optimise for retention and margin over rewrites. That is the opening for a modern, local entrant, but it is an inference, not a finding.

## 2. What the product does
Module names as the vendor's own page states them ([ghostpractice.co.za](https://www.ghostpractice.co.za/)), cross-checked with listings ([Software Finder](https://softwarefinder.com/legal/ghostpractice), [Capterra](https://www.capterra.co.za/software/213777/ghostpractice)):

| Area | Vendor wording / listing | What we infer we must match |
|---|---|---|
| Billing and trust accounting | "Manage Billing & Trust Accounting"; GL, WIP, AP/AR, e-billing formats, trust overdraft/imbalance alerts, law-society compliance reports | Fee notes, ledgers, trust ledgers, LPC-aligned reports (Phase 5 and 7) |
| Matters and documents | "Manage Matters & Documents"; document storage and capture | Matter file, document store, search (Phase 6) |
| Time and fees | "Track Time & Fees"; listings add "automated time entry with prompts for real-time fee capture" | Our wedge: capture, attribution, narrative (Phases 2–4) |
| Contacts | "Catalogue Contact Information"; CRM-style | Client and third-party registry, conflicts (Phase 6) |
| Tasks and milestones | "Manage Tasks & Milestones"; workflow templates, deadlines | Task engine, key dates (Phase 6) |
| Document assembly | "Streamline Document Assembly"; templates; **ACL5** integration | Template engine (Phase 6, human-reviewed) |
| Office 365 | Outlook, Word, Excel integration | M365 connector and Outlook add-in (Phase 2, 4) |
| Mobile / tablet | Android and iOS | Responsive web first; native later |
| Business intelligence | "Real-time business intelligence for management" | Dashboards from the ledger (Phase 5+) |
| Other | Court calendaring, e-signature, workflow automation (listing specs); **Conveyancer/Unity** integration (one reviewer) | Court calendar and e-sign are later; conveyancing integration is a market-specific must-check |

**AI claims:** none on the vendor page. Time capture is described as "automated time entry with prompts" on listing sites, which reads as prompt-driven manual entry rather than passive capture. Verify in a demo.

## 3. Commercial and technical facts
| Fact | Value | Source / caveat |
|---|---|---|
| Deployment | Windows, **on-premise** | [Capterra](https://www.capterra.co.za/software/213777/ghostpractice) (a Canadian listing); the vendor page does not state hosting |
| API | **"No API is available"** | [Software Finder](https://softwarefinder.com/legal/ghostpractice) listing |
| Pricing (Canada listing) | From CA$70 per user per month; minimum about CA$500 per month for up to 10 users (≈ R910/user at an assumed R13/CA$; confirm the rate) | Capterra; **South African pricing is "custom"** |
| Target | Firms of 5–300 people | Capterra |
| Language | English only | Capterra |
| Ratings | 4.8/5 from 11–12 reviews | Tiny sample, and likely vendor-solicited; do not treat as satisfaction data |

## 4. What users say
Positive (consistent across both listings): easy to learn, fast reports, responsive support, good migration help, paperless filing, time dockets and monthly invoicing in one application.

Complaints, which are our opening:
- **Dated interface.**
- **Weak Outlook integration** ("tighter, more seamless" wanted); **no email viewer or key-date reminders** (Software Finder).
- **Restrictive financial module:** errors are corrected with negative transactions (Software Finder). This may be correct accounting practice rather than a defect; do not market it as one without checking with an accountant.
- **Batch operations missing:** dockets can only be moved between matters one at a time.
- **Limited customisation**, and reporting across heavily customised departments needs several reports.
- **Compliance gaps called out by reviewers:** more focus wanted on **FICA and POPIA**; one reviewer raised data-security concerns.
- **Change management is hard** for staff (buy-in needed).
- **Not mentioned in any review we read:** trust accounting. Either it works well enough to be invisible, or reviewers did not use it. Unknown.

## 5. Strengths we must respect
1. **Installed base and trust.** About 10 000 users, high satisfaction scores, strong onboarding and support.
2. **Regulated money handled in one place.** Billing plus trust accounting plus law-society reports is the hard 20% that a time-capture tool does not touch.
3. **Local market fit:** SA terminology, conveyancing integration, LPC-style reporting.
4. **Familiar to staff.** Replacement risk is mostly human, not technical.

## 6. Implications for TimeLex
| Finding | Implication |
|---|---|
| No API, on-prem Windows | Integration must be through sanctioned exports/imports or the Office integration; direct database access likely breaches licence terms and is unsafe ([ADR-0009](../adr/0009-ghostpractice-interop-strategy.md)) |
| No AI, prompt-based time capture | The wedge is credible: passive capture plus attribution plus narrative can run **alongside** GhostPractice in shadow mode first |
| Strength is billing plus trust | Do not lead with a trust-accounting rewrite. It is the highest-risk, most regulated module ([research 03](03-south-africa-compliance.md)) |
| Owner is a consolidator | Expect slow product change; our window is real but not infinite |
| Hard change management | Shadow mode, import tooling and "no re-training for fee earners" matter more than features |

## 7. Open questions (to answer before Phase 5)
1. What file/report exports does GhostPractice offer for dockets, matters, contacts and ledgers, and under what licence terms?
2. Which database engine does it use on-prem, and does the licence forbid read-only reporting access?
3. How do firms actually record time today (Outlook prompts, timers, retrospective entry)?
4. What do the LPC and auditors require from a replacement ledger (see research 03)?
5. What do firms pay in South Africa, including hosting and support?

## Sources
- [GhostPractice (vendor)](https://www.ghostpractice.co.za/) · [Legal Technology Hub](https://www.legaltechnologyhub.com/vendors/ghostpractice-by-dye-and-durham/) · [Software Finder](https://softwarefinder.com/legal/ghostpractice) · [Capterra ZA](https://www.capterra.co.za/software/213777/ghostpractice) · [Capterra reviews](https://www.capterra.co.za/reviews/213777/ghostpractice) · [Dye & Durham press release](https://dyedurham.com/dye-durham-acquires-practice-management-software-provider-ghostpractice)
