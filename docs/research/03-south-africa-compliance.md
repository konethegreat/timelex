# 03 · South Africa compliance and platform constraints

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

**This is research, not legal advice.** Each section lists what a source says, what we do not know, and who must confirm. A South African attorney and a data-protection lawyer must review this file before any pilot (task **T0-L01**).

## 1. Legal Practice Act and the LPC Accounting Rules (trust and business accounts)

The Legal Practice Act 28 of 2014 and the Legal Practice Council (LPC) rules govern how firms keep trust and business accounts. Accounting rules (Part XII, rules 54 to 57) as published on [Acts Online](https://www.acts.co.za/legal-practice-act-2014/n401_accounting_rules). From the text we read:

| Rule | Requirement (paraphrased) | Why it matters to us |
|---|---|---|
| 54.6, 54.8 | Keep records of business **and** trust transactions, distinguishable "in readily discernible form" | Separate ledgers and schemas for trust vs business (Phase 7) |
| 54.9, 54.12 | Keep accounting records and client accounts at least **seven years** | Retention policy and tamper-evident storage |
| 54.10 | Write up and balance records **monthly** (deemed compliant if done by the last day of the following month) | Scheduled close and reconciliation jobs |
| 54.11 | Trust money never credited to a business account; transfers to business at least monthly, identifiable and traceable | Hard guard in the trust service |
| 54.12 | Account to clients in writing, showing receipts, disbursements, fees and charges, and balance | Fee-note content requirements (Phase 5) |
| 54.14.3, 54.14.4 | Computerised records: monthly backups kept "remote from the firm"; records retrievable in readable and printable form, including after system upgrades | Backup and export obligations on any ledger we run |
| 54.14.5 | Electronic payments need an audit trail verifying payee bank details | Controls on any payment feature |
| 54.14.7.1 | Adequate **internal controls**, designed, implemented, monitored | Maker-checker, audit log |
| 54.14.8–54.14.11 | Trust balances must cover creditors; no trust creditor may be in debit; shortfalls reported to the Council **immediately** | Real-time invariant checks and alerts |
| 54.15 | Monthly list of trust creditors reconciled to bank, investment and cash balances; lists kept seven years | Reconciliation engine |
| 54.19, 54.20, 54.24 | Partners/directors responsible; annual audit or inspection by an appointed auditor; audit report within six months of year-end (57.2) | Audit pack export |
| 57.3 | Unusual trust bank arrangements ("no interest, no charges") are misconduct | Config validation |

Method caveat: we read machine-generated summaries of the rule text, not the primary text line by line. Verify every rule number and wording against the Act before relying on it (task T0-L03). Time recording is **not addressed** in the rules we read; fee accounts (54.12) are. The LPC advisory notice confirms trust-account audits still apply after the 8 March 2024 *LSSA v CIPC* order that exempted incorporated practices from business-account audits ([LPC notice](https://lpc.org.za/wp-content/uploads/2024/05/Advisory-notice-Audit-of-LP-business-accounts.pdf)).

A vendor-sponsored article ([GoLegal](https://www.golegal.co.za/attorney-trust-audits/)) says audits are due by 31 August with a 1 March–end-February financial year. **Do not rely on it.** Confirm against rule 54.24 and the LPC.

**Consequences for the plan**
- Time capture, WIP and pro-forma billing are *not* directly regulated by Part XII. They can ship first.
- Anything that records, moves or reports trust money **is**. It is Phase 7 and gated ([PHASES.md](../PHASES.md)).
- Any ledger we run must meet 54.14.3/54.14.4 (backup, readability) and be provably append-only ([ADR-0006](../adr/0006-event-sourced-activity-and-ledger.md)).

## 2. POPIA (Protection of Personal Information Act 4 of 2013)

- **Role.** The firm is the *responsible party*. Whoever hosts TimeLex for a firm is an *operator*; a written operator agreement is required. If a firm self-hosts, the firm is both. (Standard POPIA structure; confirm with counsel.)
- **Cross-border transfer (section 72).** Sending client data to a model provider outside South Africa is a transfer. As commonly summarised, the grounds include recipient binding rules or agreements giving equivalent protection, data-subject consent, or necessity for a contract; we have not read the primary text, so confirm against section 72 itself (search results we did not open: [CMS: managing cross-border transfers](https://cms.law/en/zaf/legal-updates/Managing-cross-border-data-transfers), [PER journal article](https://perjournal.co.za/article/view/15234)). An undated practitioner blog post said the Information Regulator was "close to finalising" a transborder guidance note and that the note would be advisory, not binding ([Michalsons](https://www.michalsons.com/blog/guidance-note-on-cross-border-transfers-to-from-south-africa/77246)). **We did not find the final note. Check the Regulator's site.**
- **Provider residency.** South Africa is on Anthropic's [supported countries list](https://www.anthropic.com/supported-countries). ClearCut's [ADR-0005](https://github.com/konethegreat/clearcut/blob/main/docs/adr/0005-model-strategy-escalation.md) records that no South African or African inference region was documented and that retention terms were unverified. **Verify retention and zero-data-retention terms directly with Anthropic** (task **T0-L02**).
- **Minimisation.** Our design sends the least data that does the job: metadata first, redaction of ID/bank/phone/email before any model call (the `workers/py-worker` redaction module is the first implementation), and no tenant's data in another tenant's context.
- **Automated decisions (section 71).** Decisions with legal or substantial effect on a person based solely on automated processing are restricted. Billing a client is a consequential decision; this reinforces certification and sampling for fee notes ([AUTONOMY.md](../AUTONOMY.md)).
- **Information Officer** registration and a POPIA assessment are needed for whoever operates TimeLex commercially.

## 3. Professional confidentiality and privilege
- Captured material (emails, documents, calls) includes **privileged and confidential client communications**. Capture is therefore **opt-in per user, pausable, excludable by client/matter/domain**, and defaults to metadata-only until the firm enables content capture.
- The desktop agent filters locally before anything leaves the machine.
- Firms using their own API key remain the party sending data to the provider. We must make that explicit in the UI and contract; the platform never sees plaintext keys after storage ([ADR-0004](../adr/0004-byo-keys-and-secrets-vault.md)).
- Whether using a US-hosted model with client communications is acceptable under a firm's professional-conduct obligations is a question for the firm's counsel and, where relevant, the LPC. We do not answer it; we make the data flow inspectable and configurable.

## 4. Anthropic Usage Policy (applies to everyone building on Claude)
From the [Usage Policy](https://www.anthropic.com/legal/aup):
- **Legal high-risk use.** If a system gives "a legal High-risk AI Recommendation", meaning advising an individual on their legal rights, obligations or liability, or preparing legal documents for them to sign or file, then "a qualified person must meaningfully review" it with authority to change it, and the individual must be told AI was used. General legal information is excluded.
- **Responsibility for agents.** "Users are responsible for ensuring that agents they build or deploy" comply.
- **No unauthorised resale or proxying.** Users may not "resell, proxy, or otherwise provide access to Claude through unauthorized means", including routing through consumer subscriptions. The policy does not mention bring-your-own keys; the safe reading is that each firm uses **its own Console API key under its own agreement**, and we never route requests through any key that is not the tenant's.
- **Supported regions** apply.

**Consequences:** time entries, narratives and pro-formas are not legal advice and can be autonomous under policy. Drafting documents or advice for clients is **always** reviewed by a qualified person and labelled as AI-assisted ([AUTONOMY.md](../AUTONOMY.md) §Never autonomous).

## 5. Other items to track
| Topic | Why | Status |
|---|---|---|
| **FICA** | Reviewers asked GhostPractice for more FICA focus; client onboarding and record-keeping duties | Research in Phase 6 |
| **VAT on fees; e-invoicing** | Fee notes are tax invoices; SARS e-invoicing dates are disputed in ClearCut's research | Delegated to ClearCut ([ADR-0008](../adr/0008-clearcut-relationship.md)) |
| **Cybercrimes Act reporting duties** | Possible duty to report certain incidents | Check with counsel |
| **Electronic Communications and Transactions Act** | Validity of electronic records and signatures | Check in Phase 6 (e-sign) |
| **Professional indemnity insurance** | Insurers may ask about AI use | Firm-level; provide an "AI use statement" template |

## 6. Compliance requirements we adopt now
1. Append-only, tamper-evident ledgers; corrections by reversing entries, never edits.
2. Tenant-level retention settings with a seven-year default for financial records.
3. Capture off by default; per-user pause; per-client/matter/domain exclusions; redaction before model calls.
4. Autonomy policy cannot be enabled for any trust transaction.
5. Full audit log of agent actions: actor, tool, inputs, outputs, policy decision, model, tokens.
6. Documented data-flow diagram per tenant, exportable for the firm's POPIA records.

## Open items
| ID | Item | Owner |
|---|---|---|
| T0-L01 | SA attorney and data-protection lawyer review of this file | Kone |
| T0-L02 | Anthropic retention / ZDR / residency terms confirmed in writing | Kone |
| T0-L03 | Read rule 54.24 and the rest of Part XII; confirm software/records duties | Kone + auditor |
| T0-L04 | Check Information Regulator for the final transborder guidance note | Kone |
