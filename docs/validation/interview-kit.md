# Interview kit: T0-R01 and T0-B01

_Author: Kone Tshivhinda · Drafted 20 Oct 2026 · Written by Kone Tshivhinda & Claude_
Companion: [demo-checklist.md](demo-checklist.md). Strategy context: [research 06 §7 and §10](../research/06-strategy-and-positioning.md).

**What these interviews decide.** Whether the wedge (capture, attribute, narrate, post, pro-forma) is something firms would **pay for**, and what they pay **today, in Rand**. If it is not, we change course before Phase 2. Five interviews cannot prove demand. They can **disprove** it, and they show which assumptions to test next.

**Use this kit like this.** You run the sessions; Claude drafts and later helps synthesise. Everything here is a script, not a survey: follow the person's story, and use the question lists to make sure each topic gets covered.

---
## 1. Hypotheses and what would change our mind
Write your own prediction next to each before the first interview. Thresholds are **proposed**; change them before interview one, not after.

| # | Hypothesis | Evidence for | Evidence against (reconsider if…) |
|---|---|---|---|
| H1 | Fee earners lose billable time because recording is late or skipped | They describe reconstructing time days later; can name a matter where time was not billed | 4 of 5 say they record accurately the same day and cannot name a loss |
| H2 | A draft-and-approve tool (autonomy rung L2) is welcome, and some entries could post without review (L3) | At least one entry type they would let post unseen (for example a calendar meeting with a known client) | Nobody would let any entry post without review under any conditions: the exception-based claim has no market |
| H3 | The cost of getting from time entries to a pro-forma is real | Billing staff or partners describe manual WIP review, chasing late time | Pro-forma preparation takes them minutes and nobody calls it a pain |
| H4 | A platform fee of **R250–R450 per fee earner per month** is acceptable (hypothesis in research 06) | Unprompted ceiling above R250 from at least 3 of 5 | Ceiling below R150, or "free tool or nothing" |
| H5 | A firm can and will supply its own Anthropic key (bring-your-own-key) | Someone owns the decision, has a card or procurement route, and says "fine" | Nobody can name who would create the account, or it needs board-level approval and a long wait |
| H6 | Sending work metadata (then redacted content) to a model hosted outside South Africa is acceptable | They accept it with the controls we describe | A partner says "never, whatever the controls": note which data they would allow |
| H7 | Most billable work leaves traces in Microsoft 365 (mail, calendar, Teams, Word) | They name M365 sources for most of their time | A large share is phone, WhatsApp, in-person, court or non-Microsoft tools |
| H8 | Fee earners tolerate a capture agent or add-in being installed | IT can approve it | IT or the managing partner will not allow any installed agent or tenant-wide consent |

**Decision rule (proposed):** after 5 fee-earner interviews, continue to Phase 2 only if H1 and H3 hold for at least 3 firms **and** H2, H5 and H8 are not refuted. If H2 fails everywhere, reposition on pro-forma preparation and evidence rather than autonomy ([research 06 §10](../research/06-strategy-and-positioning.md)).

## 2. Who to interview
- **T0-B01 (5 interviews):** mix of roles: 2 fee earners (one junior, one senior), 1 partner or director who signs fee notes, 1 billing clerk or bookkeeper, 1 practice manager or IT lead. Try for at least 2 firm sizes (for example under 10 and over 25) and at least 1 firm that is **not** on GhostPractice.
- **T0-R01 (3 interviews):** three GhostPractice users from different firms, ideally one administrator, one fee earner, one billing user. Use the GhostPractice module (§7) in addition to the core script.
- Record for each firm (not for individuals): size band, whether it conducts a trust account practice ([research 03 §1](../research/03-south-africa-compliance.md), rule 54.1), hosting model, mail system.

## 3. Ground rules (read before every session)
1. **No real client data.** Do not ask to see matters, emails, dockets or trust ledgers. If a screen is shared, ask them to use a blank or training environment, or to describe it. If client names appear by accident, do not note them.
2. **Consent first, in writing or on the recording.** Say what you are doing, that it is voluntary, that notes are kept locally and shared only in anonymised form, and that they can stop or withdraw their notes. Record audio only with a clear yes.
3. **Personal information.** Keep names, firm names and contact details in one sheet, separate from the notes; use codes (F1, F2…). This is personal information under POPIA even in a research setting.
4. **No selling, no promises.** Do not claim TimeLex works, is compliant, saves money or is accurate. Say "we are deciding what to build".
5. **Past behaviour over opinions.** Ask "tell me about the last time…" and "what did you do?", not "would you like…?". Opinions about hypothetical products are cheap.
6. **Silence is a tool.** After a number or a hard answer, wait.
7. **Do not touch their software** beyond what they choose to show you. Never ask for credentials or access ([ADR-0009](../adr/0009-ghostpractice-interop-strategy.md)).
8. **Mention no employers, projects or people** from your own history in any notes or documents we share.

## 4. Core script (45 minutes)
Times are guides. Section 4.4 (money) must not be skipped.

### 4.1 Opening (3 min)
"Thank you. I am researching how South African firms record time and prepare fee notes, to decide whether a tool is worth building. I am not selling anything. There are no right answers. I will ask about how things actually work today. May I take notes, and may I record audio? You can stop me at any time."

### 4.2 Their world (5 min)
1. Tell me about your role and the kind of work you do in a typical week.
2. How many people record time at your firm? How many fee earners, how many billing staff?
3. Which systems do you use daily for mail, calendar, documents and billing? (Listen for Microsoft 365, Google, WhatsApp, phone, Teams, Zoom.)

### 4.3 How time gets recorded today (12 min)
Anchor on **the last working day** or **the last billing run**, not on "usually".
1. Walk me through yesterday. What did you work on? Which of that is recorded now?
2. For one piece of work you did recently, how did it get from "I did it" to a time entry? Who typed it? When? What did they look at?
3. How long does it typically take between doing the work and recording it? What gets forgotten?
4. Tell me about the last time time went unrecorded or unbilled. What was it? What did it cost, roughly, in Rand?
5. What do you use as a prompt or memory aid? (Outlook calendar, sent mail, diary, notes, WhatsApp, timer.)
6. How are narratives written? Who edits them? How long per entry? Do clients or insurers have rules for narratives?
7. What work never leaves a digital trace (phone calls, court, site visits, meetings in person)? What share of your day is that?
8. Has anyone tried a tool to help? What happened? Why did it stop?

### 4.4 Billing, WIP and the pro-forma (8 min)
1. What happens between "time is recorded" and "the client receives a fee note"? Who touches it, in what order?
2. Is there a pro-forma or draft stage? Who reviews it, and what do they change most?
3. How many hours a month does preparing and reviewing fee notes take, across the people involved?
4. What is the main reason a fee note goes out late or is written down?
5. How are corrections handled in your system today? (Listen for reversing entries.)
6. Do any clients or insurers set outside-counsel billing guidelines? Who enforces them?

### 4.5 Money today, in Rand (8 min) ⭐
Ask each question, then **write the number and what it covers**. If they do not know, ask who does.
1. What do you pay per month, in Rand, for your **practice-management and billing software**? Per user, or in total? Which users count?
2. What else do you pay around it: hosting or server costs, backups, IT support, add-ons, Microsoft 365, training? Roughly per month?
3. How was it priced when you bought it? Implementation or data-conversion fees? Annual increases?
4. What is your standard hourly rate for a fee earner, and a typical target for recorded hours per day or month? (A range is fine.)
5. Roughly how many hours a week per fee earner do you think go unrecorded? **How do you know?** What would that be worth in Rand at your rate? (Treat it as an estimate and note how they derived it.)
6. Have you paid for any tool or service to help with time or billing in the last two years? What, how much, and was it worth it?
7. Who decides to buy software? Who signs? How long does a decision take? Is there a budget cycle?

### 4.6 Technology, trust and approval (6 min)
1. How do you feel about client emails being read by software to help with time recording? What would need to be true for that to be acceptable? (Listen for privilege, POPIA, partner sign-off, client consent.)
2. Does your firm use any AI tool today? Under what rules? Who approved it?
3. Who in the firm could approve installing a program on staff computers, or granting a tool access to mailboxes? How long would that take?
4. Do fee earners work on their own computers, shared computers, or through remote desktop or a virtual workstation?
5. If a tool needed your firm to open its own account with an AI provider and pay usage costs directly (a few tens of Rand per person per month is the order of magnitude we expect, **not yet measured**), who would do that, and what would worry them?

### 4.7 Concept reaction (6 min)
Show the five-screen storyboard described in [demo-checklist.md](demo-checklist.md) Part B (synthetic data only), or describe it. Say only: "This is a sketch, nothing is built."
1. In your own words, what do you think this does?
2. What is the first thing you would worry about?
3. Which of these would you let happen without looking: a calendar meeting with a known client; an email thread about an open matter; a document edited for 40 minutes? Why that one?
4. What would the tool have to show you before you trusted a proposed entry?
5. Who in your firm would hate this? Why?

**Price question (ask only now, after the concept; do not name a number first).** "At what monthly price, per fee earner, would this be so cheap you would doubt it works? A bargain? Getting expensive but you would still consider it? Too expensive to consider?" Write all four numbers in Rand.

### 4.8 Commitment and close (3 min)
Offer the smallest next steps and record what they accept (this is the strongest signal in the whole interview):
1. "May I come back in a few weeks with something more concrete?" (time)
2. "Would you introduce me to your billing person or partner?" (reputation)
3. "Would you try a pilot on your own mailbox, a consented test account, for two weeks?" (access and effort)
4. "Would you sign a short non-binding letter of intent that says you would pay if it did the following?" (money; only for firms that have been enthusiastic)

"Is there anything I should have asked and did not? Who else should I talk to?" Thank them. Send a two-line thank-you within a day.

## 5. After each interview (10 minutes, same day)
Fill in the sheet. Quote, do not summarise, for any number or strong statement.

| Field | Entry |
|---|---|
| Code, date, role, firm size band, trust-account practice (y/n/unknown), hosting model, mail system | |
| Time recorded: same day / within a week / later; main prompts used | |
| Unrecorded or unbilled time: their estimate, **how they derived it** | |
| Current software spend (R per user per month; what it covers), other IT spend | |
| Hourly rate band; target hours | |
| Pro-forma process pain: hours per month, who | |
| Would let post unseen: which entry types (H2) | |
| BYO key: who would set it up, blockers (H5) | |
| Model hosted abroad: acceptable? conditions (H6) | |
| Install or tenant consent: who approves, how long, blockers (H8) | |
| Remote desktop or virtual workstation use | |
| Price: too cheap / bargain / getting expensive / too expensive (R) | |
| Commitment accepted (1–4) | |
| Best quote, worst surprise | |
| Hypotheses supported, refuted, unclear | |

## 6. Synthesis (after interviews 3 and 5)
1. Tally H1–H8 per interview: **supported, refuted or unclear**. Refuted counts more than supported.
2. Put all money numbers in one table in Rand: current spend per user, price ceilings, estimated unrecorded hours times rate. Mark **estimate** vs **observed**.
3. Compare to research 06 §7 (competitors R700–R3 000 per user per month; our hypothesis R250–R450; illustrative model spend R13–R56 per fee earner per month, **unmeasured**) and write what moves.
4. List surprises, and any new segment (for example firms that are not on GhostPractice).
5. Apply the decision rule in §1. Write the outcome in [WORKLOG.md](../WORKLOG.md) and update [TASKS.md](../TASKS.md). Do not publish individual answers.
6. State sample limits plainly: five interviews are not a market study.

## 7. GhostPractice module (T0-R01 only, add 15 minutes)
Ask in addition to the core script. Do not ask them to share client data or any credentials.
1. How is GhostPractice hosted for you: on a server in the office, cloud, a third-party IT provider, remote desktop, remote application, or a virtual workstation? Which Windows and Office versions? Who administers it?
2. How do fee earners record time in it today: prompts, timers, batch entry, mobile? What is annoying?
3. How do you use the Outlook and Word integration, if at all? What would you want it to do that it does not?
4. Which reports or exports do you run regularly? What formats (CSV, Excel, PDF, other)? Where do the files go? Have you ever imported data into it (starting data, spreadsheets, another system)? What happened?
5. Has the vendor mentioned an API, integration partners, or a data export for leaving the product? What did they say?
6. When did the firm last upgrade? How did the vendor communicate changes? Is there a user group or forum you rely on?
7. What does it cost per user per month, in Rand, and what is separate (support, hosting, add-ons)? When is it renewed?
8. How does trust accounting work for you, in general terms: who runs the month-end, how long it takes, how comfortable the auditor is? (Do not ask for figures or client names.)
9. If you ran a second tool next to GhostPractice for six months that produced ready-to-key time entries, what would stop you? Who would have to agree?
10. What would make you leave GhostPractice? What would make that impossible?

Write down what they volunteer about **licence terms** (can they run reports directly against the database, can they export freely), but do not ask them to send you the contract; request it through the vendor or a firm that agrees ([T5-L01](../TASKS.md)).

## 8. Things not to say or do
- Do not describe the product as accurate, compliant, secure or autonomous. It is none of those yet.
- Do not quote our price hypothesis. For model usage cost say only "we expect it to be small and we will measure it"; the "few tens of Rand" in §4.6 is for your orientation, not a promise.
- Do not argue with an objection. Ask "what would have to be true for that to be fine?".
- Do not collect more personal information than a professional role and a contact for follow-up.
