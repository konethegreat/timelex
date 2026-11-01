# ADR-0020: Workload identity and short-lived key leases

Status: Accepted · Date: 2026-11-01 · Deciders: Kone Tshivhinda, Claude

Builds on [ADR-0004](0004-byo-keys-and-secrets-vault.md) (BYO keys, leases and the audit chain), [ADR-0012](0012-internal-service-authentication.md) (internal tokens signed by the gateway), [ADR-0003](0003-mcp-host-and-agent-runtime.md) (the host) and [ADR-0018 point 8](0018-model-loop-money-and-failure-rules.md) (the key is a per-run lease). Implements the lease half of P1-13.

## Context

The vault holds each firm's provider keys. Until now only the gateway could reach it, with a token the gateway signs for a **user** (`kind: principal`). That works for an admin creating or rotating a key, but not for `mcp-host`: a model run is started by a user and then continues in a background host that has no user session, no API key of its own and no business holding the firm's Anthropic key for longer than one run.

Three rules constrain the answer. ADR-0012 says a service holds only public keys, so a compromised service cannot mint an identity for another. ADR-0004 says the vault is write-only except for the one caller that must present the key to a provider, and that every access belongs in the firm's hash-chained audit. ADR-0018 says the adapter is built from a leased value and that dropping it wipes the value.

The gateway cannot simply mint a token for the host. A token it signs names `kind: principal` and a user; there is no user click behind a scheduled run, and making the gateway sign arbitrary service tokens would make it a confused deputy for any tenant on request. A shared secret between the host and the vault is rejected by ADR-0012 for the same reason it was rejected there: anyone who reads it can speak for the service.

## Decision

1. **A service signs for itself, with its own key.** A workload token is a JWT with `iss: timelex-<service>`, `aud` the service being called, `kind: workload`, `svc` the service name, `tid` the firm, `sub` the initiating user and `purpose` a phrase from a fixed vocabulary. It carries no `key` and no `perms`. It is EdDSA/Ed25519, lives 30 seconds, and follows the gateway's rules exactly: at most 60 seconds, 5 seconds of clock skew, a unique `jti`.
2. **Trust is an explicit, per-service map.** A calling service is configured with its own private key; the vault is configured with `TIMELEX_WORKLOAD_PUBLIC_KEYS` as `service:kid=base64[,kid=base64][;service:...]`. Key ids are unique across services, so a key alone names the one service allowed to use it. An unknown key, an issuer that does not match the key's service, or a `svc` claim that disagrees with it is refused.
3. **Kinds never mix.** The gateway verifier accepts only its three principal kinds and refuses `workload`; the workload verifier accepts only `workload`. A workload token that carries `key` or `perms` is malformed, and a gateway token is not a workload token even if the same key is trusted for both.
4. **The vault fails closed.** With no workload key configured, every lease is 401, and a malformed configuration stops the vault at startup. There is no development bypass.
5. **Leases are by kind and by purpose.** The route is `POST /internal/v1/secrets/{kind}/lease` with no body. A purpose grants exactly one kind (`model_run` → `anthropic_api_key`), checked before any lookup so a wrong purpose cannot probe which kinds a firm has stored. There is no permission requirement: the purpose is the scope, and the set of purposes is the vocabulary a service may ask for.
6. **The lease is one response, sixty seconds, no store.** The vault opens the live secret under `FOR UPDATE`, stamps `last_used_at`, appends a `used` audit entry whose detail is `lease <service> <purpose>` and whose actor is the initiating user, and returns the value with `expiresAt` 60 seconds after issue and `Cache-Control: no-store`. There is no outbox event: a lease is an access, not a change, and a busy host must not flood the activity feed. The expiry is advisory because the vault keeps no revocable copy of what it returned; the caller must drop the value at expiry or when the run ends, whichever comes first.
7. **The surface is internal and separate.** The lease lives at `/internal/v1/...` in `contracts/openapi/vault.internal.v1.yaml`, is not routed by the gateway, and is reachable only on the private interface. The public vault contract is unchanged, so the generated SDK never grows a call that returns a secret.
8. **The first caller is `mcp-host` with `model_run`.** It signs with a key of its own, leases the firm's Anthropic key for the run, builds the provider from it and drops it. `capture-hub` will reuse the same shape with its own service name, key and purposes.

## Alternatives considered

- **The gateway mints service tokens.** It would need to sign for a service with no user request behind it, and any caller that can reach that endpoint can ask for any tenant. That is the confused deputy ADR-0012 exists to prevent. Rejected.
- **A shared secret between services.** Reading it is enough to speak for the service, and it carries no firm, user or purpose on its own. ADR-0012 rejected it for the gateway hop for the same reason. Rejected.
- **Mutual TLS alone.** It proves a machine, not a firm, a user or a purpose, and issuing and rotating certificates is more operational work than one Ed25519 key pair. Rejected as the identity mechanism; it remains a sensible transport hardening later.
- **A metadata call, then lease by secret id.** The host would learn the firm's secret ids and would need `settings:manage`, which is an admin permission a background service should not hold. The purpose table gives it exactly the one kind it needs. Rejected.
- **Lease tokens with a server-side revocation list.** The vault returns plaintext, so revocation could only stop future leases, not recall the copy already handed out. The 60-second expiry and the caller's obligation to drop it are the honest rule; a list would imply a recall that does not exist. Rejected for v1.
- **An event on every lease.** The activity feed is for changes a firm cares about; a run that leases once a minute would bury them. Audit and `last_used_at` are the record. Rejected.

## Consequences

- **Key distribution becomes an operational step.** Each calling service needs a key pair; its private key lives only in that service's environment, and its public key is listed for the vault under a globally unique key id. Rotation trusts the old and new key ids at once. The compose stack and deployment docs must carry this.
- **A compromised service can lease what its purpose grants.** The blast radius is one kind for one firm at a time, every lease is audited with the service and purpose, and removing its key from the vault's config stops it at the next start. The host never receives another kind, and the lease expires in 60 seconds.
- **The vault is on the critical path of a model run.** If it is down or its keys are misconfigured, runs stop rather than fall back to a stored key elsewhere. That is the intended failure direction.
- **The value crosses one private hop.** The vault's own logs and traces never contain it, and the response is `no-store`; the network rule that only services reach the vault now also protects the one response that carries a secret.
- **The expiry is a contract, not an enforcement.** The caller is trusted to honour it. The audit records when the value was handed out, so an incident can bound the exposure window to the run's length.
- **Revisit** when a second caller's purpose needs a kind not in the table, when the vault moves behind mutual TLS, or when a KMS-backed key wrapper (ADR-0004) lands and can shorten rotation.
