# 03 · South Africa compliance and platform constraints

_Author: Kone Tshivhinda · First researched 20 Oct 2026 · Second pass (primary sources) 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

**This is research, not legal advice.** Each section says what a source says, what we do not know, and who must confirm. A South African attorney and a data-protection lawyer must review this file before any pilot (task **T0-L01**).

## 0. What changed in the second pass (T0-L03, T0-L04)

| Area | First version said | Primary text says | Effect |
|---|---|---|---|
| Scope of Part XII | Not stated | Rule 54.1: Part XII "applies only to legal practitioners conducting a trust account practice" | We must find out which pilot firms are trust-account practices before assuming any rule applies |
| Audit report timing | "within six months of year-end (57.2)" | The six months is in **54.24**; **57.2** is the administrative fine for a late report | Citation fixed |
| Retention | "at least seven years" | Seven years **from the date of the last entry** in each book, document or file (54.9.1) | Retention clock is per record, not per financial year |
| Hosted records | Not covered | **54.9.2.1**: offsite electronic hosting is allowed if records stay confidential, are reasonably secured, and stay immediately accessible to authorised persons from the firm's office and to the Council; a service provider must be bound by contract | New requirement for any hosted ledger or file store we run. It does **not** require hosting inside South Africa (see §1.3) |
| Client accounts | "in writing, showing receipts, disbursements, fees and charges, and balance" | Rule 54.12 (as amended by Notice 626 of 2020) adds: for an **agreed fee**, the account must say it was agreed and give the amount | Pro-forma and fee-note content rule for Phase 5 |
| Payments | Only 54.14.5 | Also **54.14.15**: payments by cheque or electronic transfer only; "cellular or telephone transacting" is prohibited; payees must be designated | Constrains any payment feature |
| POPIA s71 | Billing a client is a consequential automated decision, so human certification is required | s71 only restricts decisions based **solely** on automated processing "intended to provide a profile" of a person (work performance, creditworthiness, conduct and so on), and has contract-based exceptions | Our stated reason for human gates was **overstated** (§2.1). The gate may still be right; the citation was not |
| Information Regulator transfer guidance | "Close to finalising" (undated blog) | Not published as of 20 Oct 2026; the Regulator's guidance-notes page lists six notes, none on section 72 | T0-L04 closed as "not yet published; planned" (§2.3) |

## 1. Legal Practice Act and the LPC Accounting Rules (Part XII, rules 54 to 57)

### 1.1 Sources and how we read them
| Item | Value |
|---|---|
| Instrument | Rules made by the Legal Practice Council under the Legal Practice Act 28 of 2014; "Final Rules" Notice No. 401 of 2018, Government Gazette No. 41781, 20 July 2018 ([SAICA record](https://www.saica.org.za/resources/119846), dated 20 July 2018) |
| Text we read | [Acts Online, "n401 accounting rules"](https://www.acts.co.za/legal-practice-act-2014/n401_accounting_rules) and its [beta mirror](https://beta.acts.co.za/legal-practice-act-2014/n401_accounting_rules.php), read on 20 Oct 2026 |
| Amendments Acts Online records | 54.9.2.1: Notice 191, GG 44427, 9 Apr 2021 · 54.12 and 54.15.3: Notice 626, GG 43872, 6 Nov 2020 · 54.34: Notice 664, GG 45482, 12 Nov 2021 · 54.14.16: Notice 2234, GG 49833, 8 Dec 2023 |
| Later amendments | A search found none after December 2023. A search is not proof; check the Gazette |
| **Limits of this reading** | We read a republication through a summarising fetch tool that returns short quotes, **not the Gazette PDF line by line**. Rules 54.2–54.5, 54.16–54.18, 54.21–54.23, 54.25–54.33, 55 and 56 were **not read**. Kone and the auditor must still read the Gazette text (task T0-L03 stays open for that part) |

### 1.2 What the rules say (rule, content, why it matters)
Wording is paraphrased unless in quotation marks. "V" = read in the republished text on 20 Oct 2026.

| Rule | Requirement | Status | Why it matters to us |
|---|---|---|---|
| 54.1 | Part XII applies only to practitioners "conducting a trust account practice" | V | Scope check per pilot firm |
| 54.6, 54.8 | Keep accounting records in an official language; records must "distinguish in readily discernible form" business from trust transactions | V | Separate ledgers and schemas (Phase 7) |
| 54.9.1 | Keep records and matter files at least seven years from the last entry | V | Retention policy; tamper-evident storage |
| 54.9.2, 54.9.2.1 | Records live at the main or a branch office, or for electronic records where they are ordinarily hosted; other places need Council consent. Offsite hosting: confidential, reasonably secured, immediately accessible to authorised persons from the firm's office and to the Council; contractually bind the provider | V | A hosted TimeLex that holds accounting records must support firm-office access and a provider contract. Text does not mention a South African location requirement |
| 54.10 | Update and balance monthly; writing up by the last day of the next month counts as compliance | V | Scheduled close jobs |
| 54.11 | Trust money never deposited or credited to a business account; monthly transfers of non-trust money, with identification conditions | V | Hard guard in trust service |
| 54.12 (amended 2020) | Written account to the client within a reasonable time after the mandate is performed or ends; copy kept seven years; shows receipts, disbursements, fees and charges (agreed fees flagged with the amount) and the balance owed | V | Pro-forma content (Phase 5) |
| 54.13 | Pay clients within a reasonable time; verify bank details and any change | V | Payment controls |
| 54.14.2 | Report loss, theft or destruction of records to the Council "forthwith, in writing" | V | Incident runbook must include this |
| 54.14.3 | Computerised records: monthly backups kept in a safe, fireproof place "remote from the firm" | V | Backup duty on any ledger we run |
| 54.14.4 | Guard against media loss; keep records retrievable in readable and printable form, including by converting from legacy to later systems or software | V | Export and long-term readability |
| 54.14.5 | Electronic banking for trust payments: proper audit trail including verification of payee bank details | V | Payment audit trail |
| 54.14.7.1 | Adequate internal controls, designed, implemented, operating effectively and regularly monitored | V | Maker-checker, audit log |
| 54.14.7.2–.3 | Trust money deposited intact on the day of receipt or next suitable banking day; trust-investment withdrawals promptly redeposited | V | Receipts workflow timing |
| 54.14.8–54.14.11 | Total trust money not less than total trust-creditor credit balances; no trust creditor in debit; shortfalls and debits reported to the Council immediately with explanation and proof of rectification | V | Real-time invariant checks and alerts |
| 54.14.12 | Maintain a system that prevents rule breaches when transferring trust to business money | V | Transfer guard |
| 54.14.14 | Trust withdrawals only to or for a trust creditor, or transfers to the business account for money due to the firm (conditions in 54.14.14.2) | V | Withdrawal rules |
| 54.14.15 | Cheques payable to a designated payee; payment by cheque or electronic transfer only; cellular or telephone transacting prohibited | V | Payment methods |
| 54.14.16 | Trust interest paid to the Fund through the Automated Monthly Transfer System (set deadlines); failure is misconduct | V | Interest feature (P7-06) |
| 54.15 | Monthly list of all persons for whom money is held, totalled and compared with trust bank, investment and cash balances; each list kept seven years as part of the records | V | Reconciliation engine |
| 54.19 | Partners, certain directors and certain advocates responsible for compliance | V | Product cannot shift legal responsibility; say so in UI and terms |
| 54.20, 54.24 | Appoint an auditor at the firm's expense once in each calendar year or when the Council requires; auditor's report in the prescribed form within six months of the annual closing of the records; auditor reports directly to the Council in specified cases | V | Audit pack export |
| 54.34 | Opening of practice: main office designation, banking accounts, consolidated records | V | Onboarding checks |
| 57.1–57.4 | Non-compliance is misconduct; late audit report attracts an administrative fine (57.2); abnormal trust banking arrangements are misconduct (57.3); costs recoverable (57.4) | V (paraphrase only) | Read 57.3 itself before building any bank-arrangement validation |

**Time recording is not regulated by Part XII.** The text contains no time-sheet or time-recording requirement ("time" appears only in phrases such as "within a reasonable time"). Fee accounts are covered by 54.12 and 54.14.14.2.3. This supports shipping capture and pro-forma first ([PHASES.md](../PHASES.md)), subject to counsel's confirmation that our time records do not become "accounting records" in a trust-account firm.

### 1.3 Software duties we can already list (acceptance for T0-L03, provisional)
1. Keep business and trust records distinguishable (54.8).
2. Seven-year retention measured from the last entry (54.9.1).
3. If hosted offsite: confidentiality, reasonable security, immediate access for authorised persons and the Council, provider bound by contract (54.9.2.1).
4. Monthly write-up and balance (54.10) and monthly creditor list reconciled to bank, investment and cash (54.15).
5. Monthly backups held remote from the firm (54.14.3); records readable and printable, including after migration between systems (54.14.4).
6. Audit trail on electronic trust payments, including payee verification (54.14.5); payment by EFT or cheque only (54.14.15).
7. Internal controls that are monitored (54.14.7.1).
8. Invariants: trust funds cover creditors; no debit creditor; immediate reporting (54.14.8–54.14.11); a system that blocks bad trust-to-business transfers (54.14.12).
9. Client account content including agreed-fee flag (54.12).
10. Interest to the Fund through the monthly transfer system (54.14.16).

**Open interpretation questions for the attorney and auditor** (we do not answer them):
- Do captured time entries, pro-formas or fee notes held in TimeLex count as "accounting records" or "files and documents relating to matters" under 54.9, and so trigger 54.9.2.1 and the seven-year rule?
- Does "ordinarily hosted" in 54.9.2 allow a third-party multi-tenant host in or outside South Africa, and what does the Council expect to see under 54.9.2.1?
- Is a SaaS operator a "service provider" for 54.9.2.1, and what must the contract say?

### 1.4 Audits after the 2024 order (verified)
The LPC advisory notice of 10 May 2024 states that the Act and Rules require an audit of **trust accounts**, not of the firm's business accounts, and that on 8 March 2024 the High Court (Gauteng Division, Pretoria; case 2023-062245; *Law Society of South Africa v Companies and Intellectual Property Commission*) ordered that legal practices incorporated as personal liability companies are exempt from having annual financial statements audited or independently reviewed. Trust-account audit requirements "still exist" ([LPC notice](https://lpc.org.za/wp-content/uploads/2024/05/Advisory-notice-Audit-of-LP-business-accounts.pdf), read 20 Oct 2026, including the order on its page 4). The earlier summary of this was correct.

A vendor-sponsored article ([GoLegal](https://www.golegal.co.za/attorney-trust-audits/)) gives a 31 August audit date. **Do not rely on it.** Rule 54.24 ties the report to six months from the annual closing of the records, and 54.20 lets the Council set other times.

### 1.5 Consequences for the plan
- Time capture, WIP and pro-forma billing are not directly regulated by Part XII. They can ship first, **subject to the 54.9 question above**.
- Anything that records, moves or reports trust money is Phase 7 and gated ([PHASES.md](../PHASES.md)). Gate C stands.
- Any ledger we run needs 54.14.3/54.14.4 backup and readability, provable append-only behaviour ([ADR-0006](../adr/0006-event-sourced-activity-and-ledger.md)), and a hosting contract that satisfies 54.9.2.1.
- **New:** pilot selection must record whether each firm is a trust-account practice (54.1).

## 2. POPIA (Protection of Personal Information Act 4 of 2013)

Primary text read on 20 Oct 2026: [s71](https://popia.co.za/section-71-automated-decision-making/), [s72](https://popia.co.za/section-72-transfers-of-personal-information-outside-republic/), [s21](https://popia.co.za/section-21-security-measures-regarding-information-processed-by-operator/) and [s22](https://popia.co.za/section-22-notification-of-security-compromises/), on a commentary site that reproduces the Act (page for s72 posted 2 Sep 2019, updated 30 Jan 2020). The Information Regulator's own site gave only headings for ss19–22. **We have not compared these to the Government Gazette copy of the Act.** Counsel should confirm no amendment applies.

### 2.1 Section 71: automated decision making (corrected)
- **Text (paraphrase):** a data subject is protected from decisions that have legal consequences for them or affect them to a substantial degree, where the decision is based **solely** on automated processing of personal information **intended to provide a profile** of them (work performance, creditworthiness, reliability, location, health, personal preferences, conduct).
- **Exceptions (71(2)):** the decision was taken in connection with concluding or performing a contract and the data subject's request was met, **or** suitable measures protect their legitimate interests; or a law or code of conduct sets suitable measures. Under 71(3) the measures must let the data subject make representations and require the responsible party to give enough information about the underlying logic for that.
- **What this means for us:** (a) auto-attributing time and assembling a fee note is not obviously "profiling" the client; (b) the **fee earner** is a data subject too, and auto-posted time that feeds a performance view is closer to "work performance" profiling; (c) our evidence view, undo and exception review already resemble the 71(3) measures; (d) the first version of this file and [AUTONOMY.md](../AUTONOMY.md) over-relied on s71 to justify human gates on fee notes. Those gates are a sound product and risk decision (and Anthropic's policy points the same way for legal documents, [research 07](07-anthropic-terms-and-key-custody.md)), but s71 is **not** a proven legal basis for them. Counsel to confirm. Proposed wording change: [ADR-0005 amendment](../adr/0005-autonomy-ladder-and-review.md).

### 2.2 Section 72: transfers outside the Republic (text now read)
A responsible party in the Republic may not transfer personal information about a data subject to a third party in a foreign country unless one of:
- **(a)** the recipient is bound by a law, binding corporate rules or a **binding agreement** that gives adequate protection, meaning it effectively upholds principles for processing substantially similar to POPIA's conditions **and** contains provisions substantially similar to s72 on further transfers;
- **(b)** the data subject **consents**;
- **(c)** the transfer is necessary to perform a contract between the data subject and the responsible party (or pre-contractual steps at the data subject's request);
- **(d)** the transfer is necessary to conclude or perform a contract in the data subject's interest between the responsible party and a third party;
- **(e)** the transfer is for the data subject's benefit, consent is not reasonably practicable, and the data subject would likely consent.

**Mapping to a Claude call carrying captured work data** (our reading; counsel to confirm):
- Ground (a) is the realistic route. Anthropic's DPA offers EU, UK and Swiss transfer clauses and says nothing about South Africa or POPIA ([research 07](07-anthropic-terms-and-key-custody.md)). Whether its contractual protections are "substantially similar" to POPIA, including the onward-transfer limb, is a legal judgement we cannot make.
- Ground (b) is impractical for the clients, opposing parties and third parties named in emails.
- Grounds (c)–(e) fit some data, not a general mailbox.
- Open question: is an operator (processor) abroad a "third party in a foreign country" for s72? We have not verified how the Regulator or courts read this.
- Therefore **minimisation matters most**: metadata first, redaction before any model call, and per-client exclusions ([ADR-0007](../adr/0007-tenancy-and-data-residency.md)).

### 2.3 Information Regulator guidance on transborder flows (T0-L04)
- The Regulator's [guidance-notes page](https://inforegulator.org.za/guidance-notes/), read 20 Oct 2026, lists six notes (direct marketing 3 Dec 2024; voters' information, 14 May 2024; special personal information, 28 Jun 2021; children's information; COVID-19, 3 Apr 2020; political parties, 28 Jan 2019). **None is about section 72, cloud, AI or automated decisions.** The home page showed no such item either.
- Secondary sources: [Werksmans](https://werksmans.com/south-africas-information-regulator-what-the-2025-26-annual-performance-plan-means-for-business-as-presented-to-the-portfolio-committee-on-5-may-2026/) reports the Regulator's plan to issue a Guidance Note on transfer of personal information outside the Republic (advisory, covering expected diligence and transfer-impact-assessment ideas); [Michalsons](https://www.michalsons.com/blog/guidance-note-on-cross-border-transfers-to-from-south-africa/77246) (undated) says the same; [CMS](https://cms.law/en/zaf/legal-updates/managing-cross-border-data-transfers) says the Regulator has given no adequacy criteria or list of jurisdictions. One law-firm page describes the note in the present tense; we found no matching document, so that is **unverified**.
- **Status: not published as of 20 Oct 2026; announced as planned.** Re-check every quarter.

### 2.4 Operators, security and breach (ss19–22), newly relevant
- **s21(1):** the responsible party must have a **written contract** with the operator under which the operator establishes and maintains the s19 security measures. **s21(2):** an operator with reasonable grounds to believe personal information was accessed or acquired by an unauthorised person must notify the responsible party **immediately**.
- **s22:** on reasonable grounds to believe a compromise occurred, the responsible party must notify the Regulator and the affected data subject **as soon as reasonably possible** after discovery (allowance for law-enforcement needs and for scoping and restoring systems); delay of the data-subject notice only if a public body or the Regulator says it would impede a criminal investigation; the notice must be in writing by one of the listed methods and say what happened, what is being done and what the person can do.
- **Chain gap to flag:** Anthropic's DPA promises notice of a security breach within **48 hours** ([research 07](07-anthropic-terms-and-key-custody.md)), while POPIA asks an operator to tell the responsible party "immediately". TimeLex as operator to the firm must be faster than its own upstream, so our operator agreement and incident runbook need explicit wording. Counsel to advise.
- **Consequences for us:** an operator agreement template (P2-10 or earlier), a breach runbook (also covering rule 54.14.2 for records), and a vault-compromise scenario in the threat model.

### 2.5 Roles and other POPIA points (unchanged, still unverified by counsel)
- The firm is the responsible party; whoever hosts TimeLex for it is an operator; a self-hosting firm is both.
- Minimisation: metadata first, redaction of ID, bank, phone and email before any model call (`workers/py-worker`), no tenant's data in another tenant's context.
- Information Officer registration and a POPIA assessment are needed for whoever operates TimeLex commercially.

## 3. Professional confidentiality and privilege
- Captured material (emails, documents, calls) includes **privileged and confidential client communications**. Capture is **opt-in per user, pausable, excludable by client, matter or domain**, and defaults to metadata-only until the firm enables content capture.
- The desktop agent filters locally before anything leaves the machine.
- Firms using their own API key remain the party sending data to the provider. We make that explicit in the UI and contract.
- Whether using a US-hosted model with client communications is acceptable under a firm's professional-conduct obligations is a question for the firm's counsel and, where relevant, the LPC. We do not answer it; we make the data flow inspectable and configurable.

## 4. Anthropic terms and usage policy
Moved to [research 07](07-anthropic-terms-and-key-custody.md), which covers commercial terms, retention, zero data retention, residency, the DPA, the usage policy and key custody (T0-L02). Headline points that affect this file:
- Anthropic offers **no South African or African inference region**; `inference_geo` is `global` or `us` only.
- Default API retention is 30 days; zero data retention is a per-organisation arrangement through sales, and the Fable-class models cannot use it.
- The usage policy lists "evaluating a worker's performance or discipline" as a high-risk use needing qualified-person review and disclosure. That touches fee-earner analytics, not time capture itself.

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
2. Tenant-level retention settings; seven years from the last entry for financial records.
3. Capture off by default; per-user pause; per-client/matter/domain exclusions; redaction before model calls.
4. Autonomy policy cannot be enabled for any trust transaction.
5. Full audit log of agent actions: actor, tool, inputs, outputs, policy decision, model, tokens.
6. Documented data-flow diagram per tenant, exportable for the firm's POPIA records.
7. **New:** operator-agreement template and breach runbook before any hosted tenant (s21, s22, rule 54.14.2).
8. **New:** hosted records meet 54.9.2.1 (confidential, secured, immediate access from the firm's office and for the Council, contractually bound provider) before we hold any accounting record.
9. **New:** record per pilot firm whether it conducts a trust account practice (54.1).

## Open items
| ID | Item | Owner | Status after this pass |
|---|---|---|---|
| T0-L01 | SA attorney and data-protection lawyer review of this file | Kone | Open. Questions listed in §1.3, §2.1, §2.2, §2.4 |
| T0-L02 | Anthropic retention, ZDR, residency and key custody in writing | Kone | Public terms read ([research 07](07-anthropic-terms-and-key-custody.md)); **written confirmation still open**, with questions drafted |
| T0-L03 | Read Part XII from the primary text; list software duties | Kone + auditor | **Partly closed.** Rules 54.1, 54.6–54.15, 54.19, 54.20, 54.24, 54.34, 57 read in a republication; duties listed (§1.3). Gazette text and the unread rules remain |
| T0-L04 | Information Regulator transborder guidance | Kone | **Closed as "not published; planned".** Re-check quarterly |

## Sources
[Acts Online: rules, Part XII](https://www.acts.co.za/legal-practice-act-2014/n401_accounting_rules) · [SAICA: Final Rules record](https://www.saica.org.za/resources/119846) · [LPC advisory notice, 10 May 2024](https://lpc.org.za/wp-content/uploads/2024/05/Advisory-notice-Audit-of-LP-business-accounts.pdf) · [POPIA s71](https://popia.co.za/section-71-automated-decision-making/) · [POPIA s72](https://popia.co.za/section-72-transfers-of-personal-information-outside-republic/) · [POPIA s21](https://popia.co.za/section-21-security-measures-regarding-information-processed-by-operator/) · [POPIA s22](https://popia.co.za/section-22-notification-of-security-compromises/) · [Information Regulator guidance notes](https://inforegulator.org.za/guidance-notes/) · [Werksmans on the Regulator's plan](https://werksmans.com/south-africas-information-regulator-what-the-2025-26-annual-performance-plan-means-for-business-as-presented-to-the-portfolio-committee-on-5-may-2026/) · [Michalsons](https://www.michalsons.com/blog/guidance-note-on-cross-border-transfers-to-from-south-africa/77246) · [CMS](https://cms.law/en/zaf/legal-updates/managing-cross-border-data-transfers) · [GoLegal (do not rely on)](https://www.golegal.co.za/attorney-trust-audits/)
