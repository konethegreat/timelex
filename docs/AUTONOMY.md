# Autonomy: what "fully autonomous" means in TimeLex

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_
Decision record: [ADR-0005](adr/0005-autonomy-ladder-and-review.md).

## Principle
**Autonomy is a measured property of a decision type in a tenant, not a product switch.** The system does everything it can prove it got right and escalates the rest. It must never be possible to "turn on autonomy" past what the evidence supports.

## The ladder
| Rung | Name | Behaviour | Typical use |
|---|---|---|---|
| **L0** | Observe | Capture only; nothing is proposed | First week; privacy review |
| **L1** | Suggest | Proposals shown to the fee earner; nothing posts | Pilot; after a kill switch |
| **L2** | Draft and approve | Entries drafted with evidence; fee earner approves in batches | Default for new tenants |
| **L3** | Auto-post with exceptions | Entries that meet every condition post automatically; others go to the review queue; a sample is audited | Target for most entries |
| **L4** | Autonomous to pre-bill | L3 plus WIP and pro-forma assembled automatically; fee earner or partner **certifies** the pro-forma | Mature tenants only |

There is no L5. Sending a fee note to a client, moving trust money or filing documents are outside the ladder.

## Conditions for an automatic action (all must hold)
1. **Confidence ≥ the tenant's calibrated threshold** for this decision type, client class and amount band.
2. **Deterministic checks pass:** matter open, client not flagged, rate card exists, no overlap, duration within rules, narrative passes the number guard, entry within the amount cap.
3. **Policy permits** the action at this rung for this client/matter (e.g. a client with outside-counsel guidelines may be capped at L2).
4. **Recent sampled precision ≥ the floor** (proposed: 98%) over the rolling window. If not, the tenant is demoted one rung and an admin is notified.
5. **Kill switch is off** and the key and budget are healthy.

## Never autonomous (hard-coded)
| Action | Reason |
|---|---|
| Moving or reporting **trust money** | Regulated (LPC Part XII); invariants and human sign-off required ([research 03](research/03-south-africa-compliance.md)) |
| **Sending a fee note** to a client | Consequential decision (POPIA s71); client relationship; off by default, enable per client |
| **Legal advice or documents** for a person to sign or file | Anthropic usage policy requires qualified-person review and AI disclosure |
| **Write-offs and rate changes** above a limit | Financial authority |
| **Deleting** records | Retention duty; ledgers append-only |
| **Changing autonomy settings** | Humans only, with audit |

## Earned trust: promotion and demotion
- **Promotion** L2 → L3 requires: ≥ 500 reviewed entries in the window, precision ≥ the floor with a confidence interval that clears it, zero critical errors, admin approval.
- **Demotion** is automatic on a failed audit sample, a critical error, a key or budget failure, or a model/prompt change that has not passed the evaluation gate.
- Thresholds and floors above are **proposed**; they are calibrated on the gold set ([research 04](research/04-autonomous-capture-design.md) §6).

## What a fee earner sees
- **Today:** a single screen listing exceptions grouped by cause ("could not choose between two matters", "new counterparty", "unusual duration"), each with the evidence and a one-click accept/correct.
- **Why:** every entry opens to its evidence (emails, meeting, documents) and the decision record (rule, model, confidence, threshold).
- **Control:** pause capture, exclude a client/domain, undo within a window (undo creates a reversing entry, never an edit).

## Auditing
- Weekly random sample of auto-posted entries (default 5%, higher for new clients) goes to a reviewer; results update calibration.
- A tamper-evident decision log is exportable for the firm's auditor.
- Every correction becomes a labelled example (with consent) in the tenant's evaluation set.

## Metrics we publish internally
Auto-post rate, precision at threshold, recovered minutes versus final submitted time, review minutes per fee earner per day, exception causes, model cost per entry, demotion events.

## Failure behaviour
No valid key, budget exhausted, provider outage, ClearCut unavailable, or capture connector failure → affected workflows pause, entries queue, the tenant drops to L1 for the affected decision type, and an alert is raised. Nothing is lost and nothing posts on stale or partial evidence.
