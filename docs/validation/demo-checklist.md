# One-page demo checklist: T0-R01 and T0-B01

_Author: Kone Tshivhinda · Drafted 20 Oct 2026 · Written by Kone Tshivhinda & Claude_ · Scripts: [interview-kit.md](interview-kit.md)

**Rules for both parts:** no real client data on any screen or in any note · written or recorded consent before recording or screenshots · do not touch, probe or automate GhostPractice or its database · read any NDA or licence before signing and do not agree to terms that restrict building TimeLex ([ADR-0009](../adr/0009-ghostpractice-interop-strategy.md), T5-L01) · cost figures in Rand · say "sketch, nothing built".

## Part A: GhostPractice demo (T0-R01). Observe and ask; do not operate it yourself.
Request a vendor demo ([ghostpractice.co.za/contact](https://www.ghostpractice.co.za/contact/)) or ask a friendly firm to show a **training or blank environment**. Tick as you go; write the vendor's exact words for anything marked ⭐.

| Area | What to see or ask | Why we need it | ✓ |
|---|---|---|---|
| **Time capture** ⭐ | Record one time entry from scratch: number of clicks and fields; where the "fee capture prompts" appear; timers? mobile capture? | Is it prompt-driven manual entry (our inference) or something smarter? Our wedge depends on the gap | |
| **Docket data model** ⭐ | Fields on a docket or time entry: fee earner, matter, date, units, narrative, rate, activity code, VAT, status; time unit size | Our `timelex-types` contract must map to it for shadow-mode export | |
| **Outlook and Word** ⭐ | Show the integration; what it does and does not do; add-in or separate app? | The vendor claims "seamless" Office 365 integration; users call it weak | |
| **Exports** ⭐ | Every export: dockets, matters, contacts, WIP, ledgers; formats (CSV, Excel, PDF, XML); can a user schedule or script it? Sample file with fake data | Research 01: "import/export" is advertised, scope unknown. Decides how shadow mode works | |
| **Imports** ⭐ | What can be imported (spreadsheets, dockets, matters); template files; validation and rollback | Could ready-to-key entries be imported, or must they be typed? | |
| **API or integration** ⭐ | Ask: "Is there any API, web service, reporting database access or partner programme?" | One listing says no API; the vendor is silent | |
| **Hosting** | Cloud, on-premise or third-party; SQL Server version; remote desktop, remote app, virtual workstation, browser login | Where would a desktop capture agent run? | |
| **Pro-forma and fee note** | WIP view, pro-forma stage, how a fee note is generated, how corrections work (negative transaction) | Phase 5 target; what "accepted pro-forma" means to them | |
| **Trust accounting (observe only)** | Which reports exist; reconciliation and month-end flow; LPC report names; audit-pack output; overdraft alerts | Phase 7 scope. Do not enter or request ledger data | |
| **Roles and audit** | User roles, audit log, who can edit or reverse | Maker-checker expectations | |
| **Batch operations** | Move several dockets between matters | A reported gap | |
| **Key dates and email** | Key-date reminders, email viewer | Reported gaps; vendor advertises tasks and milestones | |
| **Mobile app** | Show the Android or iOS app: fee capture, offline, what syncs to the server | A "GhostPractice 2.0" app exists on Google Play | |
| **Releases and roadmap** ⭐ | How are updates delivered? Where are release notes? Any AI plans? Any change after the owner's strategic review? | We found no public release notes; owner opened a strategic review in 2025 (unverified for 2026) | |
| **Commercials** ⭐ | Price per user per month in Rand, implementation, support, hosting, renewal; contract term; **ask for the licence or terms document** | Competitor spend benchmark; T5-L01 | |
| **Migration** | How data conversion works; what a leaving customer receives | Switching cost | |

## Part B: Showing TimeLex to fee earners (T0-B01). A five-screen storyboard, synthetic data only
Nothing is built, so use five static screens (slides or a clickable sketch) with invented firms, matters and `.example` domains. Show them in this order; stop after each and ask "What do you think this is?"

| # | Screen | What it shows | What to listen for |
|---|---|---|---|
| 1 | **Your day** | Captured events (mail threads, calendar meetings, a document session) in a timeline, nothing posted yet | Surprise, privacy reaction, "what else does it see?" |
| 2 | **A proposed entry** | One entry with the matter it picked, time, a draft narrative, and **evidence links** to the events | Do they trust the evidence? What else would they need? |
| 3 | **Exceptions inbox** | Only the cases the system is unsure about, grouped by cause, one-click accept or correct | Would they prefer this to reviewing everything (H2)? |
| 4 | **Pro-forma draft** | Time for a matter assembled into a draft pro-forma with totals and an evidence trail | Does it match what their billing staff do (H3)? |
| 5 | **Controls** | Pause capture, exclude a client or domain, autonomy level per entry type, undo | Which control would be non-negotiable? Who sets it? |

Ask the price question and the commitment ladder from [interview-kit.md](interview-kit.md) §4.7–§4.8 afterwards.

**Capture after each session (5 minutes):** hypotheses H1–H8 supported / refuted / unclear · every Rand figure with what it covers · the commitment accepted · one verbatim quote. File notes under a code (F1, F2…), never a name.
