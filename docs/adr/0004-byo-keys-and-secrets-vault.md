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

## Consequences
- Threats: vault compromise, confused-deputy calls, key exfiltration through logs or prompts; see [THREAT_MODEL.md](../THREAT_MODEL.md).
- Independent security review of the vault is a **gate** before any hosted tenant (task P8-05).
- Keys can be revoked by the customer at the provider at any time; all workflows must degrade to `needs_review` gracefully.
