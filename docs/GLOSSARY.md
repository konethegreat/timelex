# Glossary

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| Term | Meaning |
|---|---|
| **Activity event** | Immutable record of an observed work signal (email, meeting, document, desktop focus). Schema: `contracts/events/activity-event.schema.json` |
| **Time block** | A derived span of work built from one or more activity events |
| **Attribution** | Choosing the client and matter for a time block |
| **Narrative** | Fee-ready description of a time entry, with citations to evidence |
| **Time entry / docket** | A posted record of billable or non-billable time in the ledger |
| **Billing unit** | Fixed increment for charging time (commonly 6 minutes = 0.1 h); configurable |
| **WIP** | Work in progress: time and disbursements recorded but not yet billed |
| **Pro-forma** | Draft bill for review before the fee note is issued |
| **Fee note** | The invoice/statement of fees and disbursements sent to a client |
| **Fee earner** | Attorney, candidate attorney or other person who records chargeable time |
| **Realisation** | Billed and collected value as a share of standard value of time |
| **Matter** | A client engagement or file |
| **Autonomy ladder (L0–L4)** | Per-decision-type policy for how much the system may do without a human ([AUTONOMY.md](AUTONOMY.md)) |
| **Number guard** | Check that every numeral/date in a narrative exists in its cited evidence; fall back to a template if not |
| **Context pack** | The exact resources supplied to a model call (inputs, schema, rules, examples, tools), logged per call |
| **MCP** | Model Context Protocol (spec 2026-07-28 is current) |
| **MCP host / client / server** | Host runs the model loop and manages clients; client connects to one server; server exposes tools/resources |
| **BYO keys** | Bring your own API keys: the tenant supplies and pays for model and connector credentials |
| **Vault** | The service that stores and decrypts tenant secrets |
| **Outbox** | Pattern that writes an outgoing event in the same transaction as the state change |
| **LPC** | Legal Practice Council (South Africa) |
| **Part XII / Rule 54** | LPC accounting rules on business and trust accounts |
| **POPIA** | Protection of Personal Information Act 4 of 2013 |
| **FICA** | Financial Intelligence Centre Act (client due diligence and record-keeping) |
| **ZDR** | Zero data retention (an arrangement with a model provider) |
| **Shadow mode** | TimeLex captures and prepares time while the firm keeps its existing system as the record |
| **ClearCut** | Sibling project for invoices: maths, VAT, narration, export |
| **ZAR / R** | South African Rand; all costs in this repo are in Rand with the exchange-rate assumption stated |
