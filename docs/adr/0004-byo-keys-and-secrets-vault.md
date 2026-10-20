# ADR-0004: Bring-your-own keys and the secrets vault

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Builds on [research 05 §5–6](../research/05-mcp-host-and-byo-keys.md) and [research 03 §4](../research/03-south-africa-compliance.md).

## Context
Clients will load their own Anthropic API key and other API keys so the model cost is theirs. We store secrets for many tenants, so the vault is the highest-value target in the system.

## Decision
1. **`services/vault`** (Rust) is the only service that can decrypt secrets. Callers (`mcp-host`, `capture-hub`) receive a **short-lived, in-memory plaintext** via an authenticated internal call scoped to tenant and purpose; plaintext is never written to disk, logs, traces, prompts or error messages (wrapper type with redacted `Debug`/`Display`, zeroised on drop).
2. **Envelope encryption.** AES-256-GCM per-secret data key, wrapped by a tenant key-encryption key held in a cloud KMS (South African region where available) or, for self-hosters, a sealed local master key. Rotation re-wraps without re-asking the customer.
3. **Write-only API.** Secrets can be created, tested, rotated and revoked, never read back. UI shows provider, scope, last four characters and last-used time.
4. **Validation on save:** a minimal provider call confirms the key works and identifies the workspace; failures are explicit.
5. **Spend controls:** per-secret and per-workflow Rand caps enforced by `mcp-host` using its own metering, plus onboarding guidance to set Anthropic workspace limits.
6. **Provider-scoped secrets** with typed kinds (`anthropic_api_key`, `m365_oauth`, `google_oauth`, `generic_api_key`) and allowed-host lists, so a key can only be used against its provider's hosts.
7. **Access is audited** (who, which service, which workflow, when) in a tamper-evident log.
8. **No pooling or fallback to a platform key.** If a tenant has no valid key, AI workflows pause and entries queue; the platform's own key (if any) is only for the platform's demo tenant.
9. **Connector OAuth:** support (a) our multi-tenant Entra/Google app with admin consent, or (b) the firm's own app registration for strict environments. Refresh tokens are stored as secrets of kind `*_oauth`.

## Alternatives considered
- *Customers paste keys into environment variables of a self-hosted deployment.* Supported for self-hosters (the vault can use an env-sealed backend) but not acceptable for hosted multi-tenant.
- *Use a managed secrets service (AWS Secrets Manager, Vault) directly.* Reasonable for storage; we still need the typed, audited, write-only API on top. Backends are pluggable behind a `SecretStore` trait.
- *Proxy model calls through our own key and bill usage.* Rejected: contradicts the brief, adds credit and reseller risk, and conflicts with the usage-policy guidance in research 03 §4.

## Proposed amendment (20 Oct 2026, after T0-L02 desk research; **not yet accepted; the Decision above is unchanged**)
Evidence: [research 07](../research/07-anthropic-terms-and-key-custody.md). Public terms neither allow nor forbid a platform holding a customer's key; Anthropic documents identity-backed keys, per-workspace spend limits and **Workload Identity Federation (WIF)**.
1. **Two credential modes** for Anthropic behind the same traits: (a) a **service-account key scoped to one dedicated workspace**, with an expiry and a workspace spend limit, in the write-only vault (point 3 above stays); (b) **WIF** with a TimeLex-run OIDC issuer, so TimeLex stores **no** Anthropic secret. Mode (b) is preferred if Anthropic confirms a vendor-run issuer is acceptable.
2. **Do not request or store** legacy workspace keys or Admin keys.
3. **Do not build WIF yet.** It moves the crown jewel from the vault to the issuer signing key (KMS-held, rotated). Wait for Anthropic's written answer (research 07 §7.3).
4. Onboarding must tell the firm, in plain words: default retention is 30 days, flagged content up to 2 years even with ZDR, ZDR needs the **firm's own** contract with Anthropic sales, and Fable-class models cannot use ZDR.
5. Onboarding asks the firm to set `allowed_inference_geos` and a workspace spend limit; our health check verifies both and degrades to `needs_review` if they are missing.
6. Spend caps and metering: reconcile against the Anthropic Usage and Cost API by workspace when available (T0-A02).
**Why a proposal, not an edit:** it changes the vault's job and adds a second credential path; that is your call.

## Consequences
- Threats: vault compromise, confused-deputy calls, key exfiltration through logs or prompts; see [THREAT_MODEL.md](../THREAT_MODEL.md).
- Independent security review of the vault is a **gate** before any hosted tenant (task P8-05).
- Keys can be revoked by the customer at the provider at any time; all workflows must degrade to `needs_review` gracefully.
