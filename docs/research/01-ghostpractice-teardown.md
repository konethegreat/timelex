# 01 · GhostPractice teardown

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

**Confidence:** medium-low. Everything below comes from third-party listings, the vendor's public website and the acquirer's press release. We have not seen the product, its documentation or a demo, and **we found no public release notes, knowledge base, user forum or export specification** (the vendor's support page lists only a phone number and an email address). Task **T0-R01** (get a demo or trial and interview three GhostPractice firms) is the first validation step, and the numbers in this file must be treated as hypotheses until then.

**Second pass, 20 Oct 2026:** every claim was re-checked against the vendor's own pages and the primary press release. Result: several claims were refined or contradicted. Read the claim register in **§8** first; where it disagrees with §2–§7, §8 wins. No GhostPractice software, database or customer system was touched.

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
| Deployment | ~~Windows, on-premise~~ **Refined (§8):** vendor FAQ says it does not host customer data; the firm chooses cloud, on-premise or a third-party IT provider; database is Microsoft SQL. "Windows" is unconfirmed | Vendor FAQ (primary). Software Finder says "primarily an on-premises solution" |
| API | "No API is available" | [Software Finder](https://softwarefinder.com/legal/ghostpractice) FAQ (updated 22 Jun 2026): "No, GhostPractice does not have an API." **The vendor's pages say nothing about an API either way**, but do advertise import/export (§8) |
| Pricing (Canada listing) | From CA$70 per user per month; minimum about CA$500 per month for up to 10 users (≈ R910/user at an assumed R13/CA$; confirm the rate) | Capterra, **not re-verified (our fetch was blocked)**; vendor FAQ says no upfront cost and a monthly rental quoted after discovery; **South African pricing is "custom"** |
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
1. What file/report exports does GhostPractice offer for dockets, matters, contacts and ledgers, and under what licence terms? **Narrowed:** the vendor advertises "Import/Export functionality to send or receive data" and spreadsheet import for starting data. Formats, scope and licence terms are unknown. Ask at the demo ([demo checklist](../validation/demo-checklist.md)).
2. ~~Which database engine does it use on-prem?~~ **Answered by the vendor FAQ: Microsoft SQL.** Version unknown. Still open: does the licence forbid read-only reporting access? (T5-L01; we do not need to touch the database to ask.)
3. How do firms actually record time today (Outlook prompts, timers, retrospective entry)? ([interview kit](../validation/interview-kit.md))
4. What do the LPC and auditors require from a replacement ledger (see research 03)? **Partly answered** in research 03 §1.
5. What do firms pay in South Africa, including hosting and support? Vendor says monthly rental, quoted after discovery. Interview question.
6. **New:** many firms may reach GhostPractice through remote desktop, remote application or a virtual workstation (all listed in the vendor FAQ). Where would a desktop capture agent run for those users?

## 8. Claim register (re-checked 20 Oct 2026)

**Labels.** **Verified** = confirmed in a primary source (the vendor's own page or the acquirer's release) · **Verified as a claim** = the vendor says it; we did not test it · **Unverified** = only in third-party listings or reviews · **Contradicted** = a primary source says otherwise · **Refined** = partly right.
Vendor pages read: [home](https://www.ghostpractice.co.za/), [fee earners](https://www.ghostpractice.co.za/fee-earners/), [accounting](https://www.ghostpractice.co.za/accounting-administration/), [FAQ](https://www.ghostpractice.co.za/faq/), [support](https://www.ghostpractice.co.za/support/), [about](https://www.ghostpractice.co.za/about/). Press release: [Dye & Durham, 23 May 2023](https://dyedurham.com/dye-durham-acquires-practice-management-software-provider-ghostpractice). Listings: [Software Finder](https://softwarefinder.com/legal/ghostpractice) (updated 22 Jun 2026), [Legal Technology Hub](https://www.legaltechnologyhub.com/vendors/ghostpractice-by-dye-and-durham/). Capterra returned HTTP 403 to our fetch, so Capterra-only facts could not be re-checked.

| # | Claim in our earlier files | Status | Evidence |
|---|---|---|---|
| 1 | Acquired by Dye & Durham, announced 23 May 2023, terms undisclosed | **Verified** | Press release |
| 2 | About 10 000 users, about 110 employees, mostly small and medium firms | **Verified as a claim** | Press release (the acquirer's statement, not an audit) |
| 3 | Largest legal practice management provider in South Africa | **Verified as a claim** | Press release, same caveat |
| 4 | "Developed by Korbitec in Cape Town" | **Refined** | Press release names **GhostPractice Inc., headquartered in Cape Town**, and does not mention Korbitec. Korbitec as developer rests on Legal Technology Hub and a 2008 article. The vendor site footer says "GhostPractice, Inc." and a PAIA file is named "Korbicom" |
| 5 | Presence in Canada | **Verified as a claim** | Press release ("also serves law firms in Canada"). The .co.za site still says "Canada's leading law practices", so it appears to reuse Canadian copy |
| 6 | Legal Technology Hub's profile (HQ Canada, North America only, founded 1874) | **Contradicted** | Press release says HQ Cape Town, South Africa. Do not cite that profile for facts |
| 7 | Owner is a consolidator and will change slowly | **Inference, now weaker** | Dye & Durham opened a **review of strategic alternatives in July 2025** (sale, asset sales or merger possible), per press coverage in search results we did not open. We found no 2026 news. Ownership may change, which is both a risk and an opening. Verify before relying on it |
| 8 | Module list: billing and trust accounting, matters and documents, time and fees, contacts, tasks and milestones, document assembly, business intelligence | **Verified as a claim** | Vendor home, accounting and fee-earner pages |
| 9 | Office 365 integration (Outlook, Word, Excel) | **Verified as a claim** | Fee-earner page: "Seamless office integration with Office 365 - Outlook, Word, and Excel." No detail given |
| 10 | Integrates with ACL5 for document assembly | **Unverified** | Legal Technology Hub only. The vendor pages instead say it can "integrate with your Cost Recovery System; Conveyancing System". Conveyancer/Unity remains one reviewer's comment |
| 11 | Time capture is "automated time entry with prompts" | **Verified as a claim, interpretation open** | Vendor: "fee automation and fee capture prompts in real-time". No timer, passive capture or AI is mentioned. That it is prompt-driven manual entry is **our inference** |
| 12 | No AI claims | **Verified for the six pages read** | None of them mentions AI |
| 13 | Windows, on-premise only | **Refined, partly contradicted** | FAQ: GhostPractice "does not currently host" customer data; hosting "can be cloud, on-premise, or through a third-party IT provider"; runs on a **Microsoft SQL** database. Remote options listed: web browser login, VPN, remote desktop, remote application, virtual workstation, mobile app. "Windows" unconfirmed |
| 14 | "No API is available" | **Unverified** | Software Finder FAQ says so. Vendor pages are silent on any API |
| 15 | Integration must be through sanctioned exports and imports, and none is known | **Contradicted at the claim level** | The vendor advertises "Import/Export functionality to send or receive data" and spreadsheet import. **What it exports, in what format, and under what licence is unknown** |
| 16 | Mobile for Android and iOS | **Verified** | A Google Play listing "GhostPractice 2.0" exists (version 3.0.130, 8 May 2026, as shown in a search result; the store page itself was not opened) |
| 17 | Pricing from CA$70 per user per month, minimum about CA$500 for up to 10 users | **Unverified, not re-checkable** | Capterra blocked our fetch. Vendor FAQ: no upfront cost, free implementation, monthly rental quoted after discovery. One reviewer says per-user monthly. South African price is "custom" |
| 18 | Targets firms of 5–300 people | **Verified as a claim** | Vendor FAQ |
| 19 | English only | **Unverified** | Software Finder FAQ |
| 20 | Rated 4.8/5 from 11–12 reviews | **Verified as a listing figure** | Software Finder: 11 reviews, 4.8, all positive. Too small and probably vendor-prompted. [Research 06](06-strategy-and-positioning.md) calls GhostPractice's scores "high satisfaction"; that over-reads the evidence |
| 21 | Dated interface; weak Outlook integration; no email viewer or key-date reminders; restrictive financial module; one-at-a-time docket moves; limited customisation; FICA/POPIA gaps; hard change management | **Unverified** | Listings and reviews only. Software Finder's cons list repeats limited customisation, no email viewer or key-date reminders, restrictive financial module. **Contested:** the vendor advertises task and milestone management with deadlines |
| 22 | Trust accounting: overdraft and imbalance alerts; trust ledger, general ledger and creditors ledger; trust-audit preparation | **Verified as a claim** | Vendor accounting page ("Client/Matter Trust Ledger; General Ledger and Business Creditors Ledger"; "effortless preparation for your Trust Audit") |
| 23 | Law-society compliance reports | **Unverified** | Legal Technology Hub only. The vendor accounting page does **not** mention LPC or law-society compliance |
| 24 | Strong onboarding and migration help | **Verified as a claim** | FAQ: free data conversion plan, dedicated project manager, spreadsheet import. Reviews repeat it |
| 25 | Database engine unknown | **Answered** | Microsoft SQL (FAQ); version unknown |
| 26 | Release notes, forum, export documentation exist publicly | **Not found** | None on the vendor site; support page has phone and email only; searches for forum or complaint threads found nothing (search coverage is limited). The "Terms of Use" footer link points to a PAIA PDF, so we found **no public licence terms** (needed for T5-L01) |

### What this changes
1. **ADR-0009's premise is weaker than written** ("no public API" and "no sanctioned export" were assumptions). The vendor advertises import/export. Shadow mode may be easier than assumed, or the export may be thin. The demo must establish it ([demo checklist](../validation/demo-checklist.md)). Proposed amendment in [ADR-0009](../adr/0009-ghostpractice-interop-strategy.md).
2. **"Windows on-premise" is not safe to build on.** Firms may run it in the cloud, through a third-party IT host, or through remote desktop. The Windows desktop agent (P2-06, T2-R03) must be tested in remote-desktop and virtual-desktop sessions before we rely on it. Add to the demo and interview kits.
3. **Ownership risk and opportunity:** a strategic review at the owner means product direction may change. This does not alter the wedge; it is a reason to keep GhostPractice-specific code behind the adapter interface (ADR-0009 point 3), as already decided.
4. **Soften "high satisfaction" and "strong installed base" wording** in research 06 until interviews measure it. The 10 000-user figure is the owner's own.

## Sources
- [GhostPractice (vendor)](https://www.ghostpractice.co.za/) · [Legal Technology Hub](https://www.legaltechnologyhub.com/vendors/ghostpractice-by-dye-and-durham/) · [Software Finder](https://softwarefinder.com/legal/ghostpractice) · [Capterra ZA](https://www.capterra.co.za/software/213777/ghostpractice) · [Capterra reviews](https://www.capterra.co.za/reviews/213777/ghostpractice) · [Dye & Durham press release](https://dyedurham.com/dye-durham-acquires-practice-management-software-provider-ghostpractice)
