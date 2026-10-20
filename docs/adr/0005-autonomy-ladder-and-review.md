# ADR-0005: Autonomy ladder and exception-based review

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Details in [AUTONOMY.md](../AUTONOMY.md).

## Context
The brief is "fully autonomous with minimal human input". Competitors review every draft. POPIA section 71 and Anthropic's legal high-risk rules restrict solely automated consequential decisions and require qualified-person review for legal advice and documents ([research 03](../research/03-south-africa-compliance.md)).

## Decision
1. Autonomy is a **per-tenant, per-decision-type policy** on a five-rung ladder (L0 observe → L4 autonomous to pre-bill), set by an administrator and capped by system rules.
2. A decision auto-executes only if **confidence ≥ calibrated threshold AND all deterministic checks pass AND policy allows AND the tenant's recent sampled precision is above its floor**.
3. **Earned trust:** promotion up a rung requires measured precision over a rolling window; **a failed audit sample demotes automatically**.
4. **Never autonomous (hard-coded, not configurable):** moving trust money; sending a fee note to a client unless the firm explicitly enables it per client; signing or filing legal documents; advising a person on legal rights; changing rates or write-offs above a configured limit; deleting records.
5. Every decision stores its **evidence, model, prompt version, confidence, threshold, policy and outcome**; corrections supersede, never overwrite.
6. A **kill switch** per tenant and per workflow drops autonomy to L1 immediately.
7. Sampling audits: a configurable percentage of auto-posted entries is routed to a reviewer; results feed calibration.

## Alternatives considered
- *Review-everything drafts (market norm).* Safe, but removes the product's reason to exist.
- *Single global confidence threshold.* Ignores differing risk by client, matter type and amount.
- *Let the model decide when to ask.* Self-reported confidence is poorly calibrated; policy lives outside the model.

## Consequences
- Needs an evaluation harness and labelled data before autonomy is enabled for any tenant (Phase 3 gate).
- Review UI must be fast: exceptions grouped by cause, one-click accept/correct, batch operations.
- Marketing language must say "exception-based" and cite measured numbers only.
