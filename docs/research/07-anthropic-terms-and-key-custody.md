# 07 · Anthropic terms, retention, residency and key custody

_Author: Kone Tshivhinda · Researched 20 Oct 2026 (task T0-L02, public documents only) · Written by Kone Tshivhinda & Claude_

**Research, not legal advice, and not written confirmation from Anthropic.** Everything below comes from Anthropic's public pages read on 20 Oct 2026. T0-L02 asks for the key-custody pattern **in writing**; that is still open, and §7 lists the questions to send. Exchange-rate assumption where money appears: R18/USD.

## 1. Documents read (with the version we saw)
| Document | Version shown when read | URL |
|---|---|---|
| Commercial Terms of Service | Page header "Effective June 17, 2025" (check for a newer version) | [anthropic.com/legal/commercial-terms](https://www.anthropic.com/legal/commercial-terms) |
| Data Processing Addendum | "Effective February 24, 2025" | [anthropic.com/legal/data-processing-addendum](https://www.anthropic.com/legal/data-processing-addendum) |
| Usage Policy | Page header "Effective November 12, 2026", which is **after today**. Treat it as an announced or upcoming version and re-read on that date | [anthropic.com/legal/aup](https://www.anthropic.com/legal/aup) |
| Retention for commercial organisations | "Last updated: July 1, 2026" | [privacy.claude.com article 7996866](https://privacy.claude.com/en/articles/7996866-how-long-do-you-store-my-organization-s-data) |
| API and data retention (ZDR) | Undated | [platform.claude.com: API and data retention](https://platform.claude.com/docs/en/manage-claude/api-and-data-retention) |
| Data residency | Undated | [platform.claude.com: data residency](https://platform.claude.com/docs/en/manage-claude/data-residency) |
| Authentication, workspaces, Workload Identity Federation | Undated | [authentication](https://platform.claude.com/docs/en/manage-claude/authentication) · [workspaces](https://platform.claude.com/docs/en/manage-claude/workspaces) · [WIF](https://platform.claude.com/docs/en/manage-claude/workload-identity-federation) |
| Supported regions | Undated; **South Africa is listed** | [supported-regions](https://platform.claude.com/docs/en/api/supported-regions) |

Our fetch tool summarises pages, so clause numbers and quotes below are as reported by it; confirm against the pages before relying on any clause in a contract.

## 2. Commercial terms (what they say that matters)
| Topic | Finding | Clause as reported |
|---|---|---|
| Customer | An organisation with authority to bind it; "not for consumer use" | Preamble |
| Ownership | Customer retains rights to inputs and owns outputs | §B |
| Training | "Anthropic may not train models on Customer Content from Services" | §B |
| Confidentiality | Customer Content is the customer's Confidential Information; recipient uses reasonable care | §E.1–E.2 |
| Responsibility | Customer is responsible for **all activity under its account** and must report suspected compromise | §D.5 |
| Building on the service | Customer may use the services to power products and services it makes available to **its own customers and end users** | §A.1 |
| Resale | Customer may not resell except as expressly approved by Anthropic | §D.4 |
| Output reliance | Customer must tell users factual assertions in outputs should not be relied on without checks | §D.3 |
| Usage Policy | Incorporated by reference | §D.2, §M.9 |
| Liability | Services "AS IS"; consequential damages excluded; cap is fees paid in the previous 12 months | §L |
| Data processing | Governed by the DPA | §C |

**What the terms do not say:** nothing in the pages we read addresses a **third-party platform holding a customer's API key** for the customer. It is neither allowed nor prohibited in the text. The usage policy forbids reselling, proxying, or providing access "through unauthorized means", and says nothing specific about key sharing. A platform that calls Claude **only with the customer's own credential, for the customer's own use, under the customer's own account** looks consistent with §A.1 and §D.5, but that is our reading, not Anthropic's. That is why written confirmation matters.

## 3. Retention and zero data retention (ZDR)
| Question | Answer (source) |
|---|---|
| Default retention of API inputs and outputs | Automatically deleted from Anthropic's backend **within 30 days** of receipt or generation (privacy centre, updated 1 Jul 2026) |
| Longer-retention exceptions | Services under the customer's control with longer retention (for example the Files API), a separate agreement, enforcement of the Usage Policy, legal compliance |
| Flagged content | If automated systems flag content as violating the Usage Policy, inputs and outputs may be kept **up to 2 years** and trust-and-safety classification scores up to **7 years**. This applies **even with ZDR** (API-and-data-retention page) |
| What ZDR is | Anthropic "does not store customer prompts or responses at rest after the API response is returned" |
| How to get it | **Contact Anthropic sales.** It is enabled **per organisation**; a new organisation needs it enabled again, and it does not carry across organisations in one account |
| Covered | Messages and Token Counting endpoints for eligible features (see the page's feature table) |
| Not covered | Features that store data (Batch API, Files API, code execution), Console and Workbench, Claude for Excel |
| **Models excluded from ZDR** | **Claude Fable 5.1, Claude Mythos 5.1, Claude Fable 5 and Claude Mythos 5** "require 30-day data retention and are not available under ZDR unless expressly authorized by Anthropic". A request to Fable 5 from an organisation whose retention setting does not meet this returns `400 invalid_request_error` |
| Workaround | An organisation with ZDR can enable 30-day retention **for one workspace only**, and use that workspace for these models; other workspaces stay ZDR. The Default Workspace follows the organisation setting, so use a different workspace |
| HIPAA | A separate "HIPAA-ready" arrangement exists; not relevant to us |

**Contradiction with our design.** [ADR-0003](../adr/0003-mcp-host-and-agent-runtime.md) and CLAUDE.md put Fable 5.1 at the top of the escalation ladder. A firm that obtains ZDR **cannot** use that rung without a 30-day-retention workspace. See the ADR-0004 and ADR-0003 amendments.

**Consequence for POPIA and privilege.** A firm may reasonably want ZDR for privileged material. ZDR requires the **firm** to contract with Anthropic sales; we cannot supply it. The default we can promise truthfully is "30-day retention, and up to 2 years if flagged". We must not advertise zero retention.

## 4. Residency
| Fact | Source |
|---|---|
| `inference_geo` accepts only `"global"` (default) or `"us"` | data-residency page |
| Workspace geo (at-rest storage and endpoint processing) is `"us"` only and cannot be changed after creation | same |
| US-only inference costs **1.1×** the standard token rates (a 10% premium) | same |
| `allowed_inference_geos` and `default_inference_geo` can be set per workspace in the Console or the Admin API | same |
| Supported only on Claude 4.6 and later models; older models return a 400 | same |
| South Africa is on the supported-countries list (access allowed) | supported-regions |
| **No South African or African inference region exists** at Anthropic | data-residency page lists none |
| Amazon Bedrock, Cape Town (`af-south-1`): Anthropic's Bedrock page lists it for **global** endpoints only, not regional ones, as reported by our search tool. Global endpoints may route to other regions. **Not confirmed on the AWS pages** | [Claude in Amazon Bedrock](https://platform.claude.com/docs/en/build-with-claude/claude-in-amazon-bedrock) (secondary read) |

So [ADR-0007](../adr/0007-tenancy-and-data-residency.md) point 2 ("hosted deployments run in a South African region for data at rest; inference location is a separate, disclosed property") is **accurate and must be disclosed plainly**: model inference leaves South Africa, and a South African data-at-rest region is a TimeLex-side choice only. An "in-region inference" option does not exist today from Anthropic; firms that need it need a self-hosted or other-provider model behind the provider trait.

## 5. The DPA (effective 24 Feb 2025)
- Anthropic is the **processor** and the customer the controller (B.1); processing beyond instructions only where required by law (B.2).
- **Transfers:** EU Standard Contractual Clauses (Modules Two and Three), a UK Addendum and a Swiss Addendum (Schedule 3). **South Africa and POPIA are not mentioned**; the DPA refers to "Applicable Data Protection Laws" generically (A.1). Anthropic will support transfer impact assessments (I.4).
- **Sub-processors:** list at [anthropic.com/subprocessors](https://www.anthropic.com/subprocessors) (Schedule 4); reasonable notice of a new one; **15 days** to object on reasonable privacy or security grounds, otherwise deemed consent (C.3).
- **Breach notice:** "without undue delay, but in any event within 48 hours" (G.1).
- **Deletion:** within 30 days after termination, subject to legal and misuse exceptions (H.1).
- **Audit:** annual external audits with reports (for example SOC 2) on request; customer audits on request, at the customer's cost, generally once in 12 months (F.1–F.4).
- **Data-subject requests:** forwarded to the customer, with reasonable assistance (D.1–D.2).
- Third-party summaries say the DPA is incorporated automatically into commercial terms; we only saw §C of the terms referring to it.

**Gap for POPIA s72(1)(a):** the contractual protections are real, but nothing shows Anthropic has agreed provisions "substantially similar" to s72 on onward transfers **for POPIA**. See [research 03 §2.2](03-south-africa-compliance.md). Ask Anthropic whether it will add a South African transfer addendum.

## 6. Usage policy (version effective 12 Nov 2026 as displayed)
| Topic | What it says |
|---|---|
| **Legal** high-risk | Advising an individual on "their legal rights, obligations, or liability", and preparing legal documents for them to sign or file |
| **Employment** high-risk (new to our analysis) | Screening, ranking or rejecting candidates, and **evaluating a worker's performance or discipline** |
| Requirements for every high-risk area | A **qualified person must meaningfully review** the recommendation before it is delivered or used, with authority to change it and remaining responsible; the individual must be **clearly told AI was used** (Anthropic need not be named) |
| Agents | "Users are responsible for ensuring that agents they build or deploy" comply, including actions through tools and connected systems |
| Resale and proxying | Prohibits reselling, proxying or providing access to Claude "through unauthorized means", including via consumer subscriptions or misrepresenting the client in use |
| Key sharing | **No specific rule found** |

**What this changes**
1. Time entries, narratives and pro-formas are not legal advice, so autonomy under policy is unaffected. Unchanged from the first pass.
2. **Per-fee-earner performance views** (utilisation, "review minutes per fee earner per day", the planned `insights` service, P8-03) are close to "evaluating a worker's performance". Keep them human-reviewed, disclose AI involvement, and do not auto-act on them. This is a new constraint on Phase 8 and on the metrics in [AUTONOMY.md](../AUTONOMY.md).
3. Document drafting for clients stays gated with qualified-person review and AI disclosure.

## 7. Key custody: options, a better pattern, and questions for Anthropic

### 7.1 What Anthropic offers (all documented, none tested by us)
| Mechanism | Detail |
|---|---|
| **Key types** | Personal key, **service account key** (identity-backed; stops when the account is archived), and legacy workspace key. Anthropic says to prefer identity-backed keys over workspace keys |
| **Expiry** | Chosen at creation: 3 hours, 1, 7 or 30 days, custom, or never; an organisation can cap maximum expiry |
| **Scoping** | A key can be scoped to a single workspace |
| **Workspace limits** | Per-workspace monthly **spend limits** with alerts, and rate limits; limits cannot be raised above the organisation's; up to 100 workspaces by default |
| **Usage and cost reporting** | A Usage and Cost API by workspace exists (needs an Admin key). Relevant to T0-A02 (reconciling our metering with the bill). **Not tested** |
| **Workload Identity Federation (WIF)** | A workload exchanges a short-lived JWT from an OIDC issuer the customer trusts for a short-lived Anthropic access token (RFC 7523 jwt-bearer). The customer creates three resources in their organisation: a **service account**, a **federation issuer** (issuer URL and JWKS source) and a **federation rule** (match on subject prefix, audience, claims or a CEL condition; target service account; scope; token lifetime 60 s to 24 h). Access token lifetime is capped at twice the remaining life of the JWT; JWTs carrying `jti` are single-use. "There is no `sk-ant-api...` string to mint, distribute, or rotate" |

### 7.2 Thought-partner proposal: prefer federation to storing keys
[ADR-0004](../adr/0004-byo-keys-and-secrets-vault.md) assumes we store each firm's Anthropic key in an envelope-encrypted vault. WIF offers a way to **hold no Anthropic secret at all**: TimeLex runs an OIDC issuer; each firm's admin creates an issuer, service account and rule that trust it, pinned to that firm's tenant claim and to a dedicated workspace with a spend cap. The platform then mints a short-lived JWT per call or per job.

| | Stored key in vault (today's ADR) | WIF with a TimeLex-run issuer |
|---|---|---|
| Secret at rest at TimeLex | Yes: highest-value target | None for Anthropic (connector OAuth tokens still need the vault) |
| Customer revocation | Delete or disable the key | Delete or disable the rule or issuer; takes effect within the token lifetime |
| Blast radius if TimeLex is compromised | Exfiltrated keys work until revoked | An attacker with the **issuer signing key** can mint tokens for every firm that trusts it, until each firm removes the rule. The signing key becomes the crown jewel (KMS-held, rotated) |
| Customer setup effort | Paste a key | Three Console resources per firm (or Admin API); needs an Anthropic org admin |
| Fit with Anthropic's guidance | Documented, works everywhere | Documented for "an identity provider you already operate"; **a vendor-run issuer is not described as an intended pattern** |
| Per-customer isolation | By workspace and key | By rule claims; each firm pins its tenant claim |

**Recommendation (Proposed, not yet an ADR change):** support **two credential modes** behind the same `SecretStore` and `ModelProvider` traits: (1) stored **service-account key scoped to one dedicated workspace, with an expiry and a spend limit**, in the write-only vault; (2) **WIF** as the preferred mode once Anthropic confirms a vendor-run issuer is acceptable. Never request or store legacy workspace keys or Admin keys. Do not build WIF until the answer is in writing; it costs a signing-key design.

### 7.3 Questions to send to Anthropic (T0-L02)
Send to the commercial or partnerships contact, in writing, and keep the reply in `docs/research/`:
1. A software platform stores a customer's Anthropic service-account key (or runs an OIDC issuer the customer trusts under WIF) and calls the API **only** on that customer's behalf, in the customer's workspace, under the customer's terms. Is this permitted under the Commercial Terms and Usage Policy, including §D.4 (resale)? Do you require a partner agreement?
2. Is a vendor-operated OIDC issuer acceptable for WIF, or must the issuer be operated by the customer?
3. Who is responsible under §D.5 for activity under a key that a platform holds? Do you recommend any scope, expiry or allowlist controls for this pattern?
4. Will you add a **South African (POPIA) transfer addendum**, or confirm which clauses of the DPA you consider give "adequate protection" for POPIA s72(1)(a)?
5. Is any in-region inference for South Africa or Africa planned, and is Bedrock `af-south-1` a regional or only a global endpoint for the current models?
6. Will ZDR be offered to a small firm's organisation, with what minimums? Can **Fable-class models** ever run under ZDR, and what is the retention for a workspace opted in to 30 days?
7. If content is flagged, can the customer be told, and can flagged-content retention (up to 2 years) be limited for privileged material?
8. Which version of the Usage Policy and Commercial Terms is current, given the policy page shows an effective date of 12 Nov 2026?
9. Can Usage and Cost API data be exposed to the customer's own dashboard so a platform can reconcile its metering without holding an Admin key (T0-A02)?

## 8. Verdict by claim
| Claim in earlier docs | Result |
|---|---|
| South Africa is a supported country | **Verified** |
| Anthropic retention and ZDR terms unverified | **Now documented** (30 days; ZDR per organisation via sales; flagged content up to 2 years even under ZDR) |
| Legal high-risk rule needs qualified-person review and AI disclosure | **Verified**, and the policy now also lists employment performance evaluation |
| Usage policy "does not mention bring-your-own keys" | **Still true**; also no key-sharing rule found |
| "Fable 5.1" can sit on the escalation ladder | **Contradicted for ZDR tenants** (see §3) |
| Residency "in a South African region where available" for the vault KEK | **Not checked** here (cloud KMS region availability is a separate check) |
| Retention of provider-side data is outside our control (threat-model residual risk) | **Narrowed**: we now know the defaults and the flagged-content exception |
