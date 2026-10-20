# ADR-0012: Internal service authentication and gateway routing

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude
Builds on [ADR-0001](0001-service-boundaries-and-contract-first.md), [ADR-0010](0010-messaging-and-sync-contracts.md) and [ADR-0011](0011-tenancy-enforcement-and-database-roles.md). Wire details: [contracts/internal-auth.md](../../contracts/internal-auth.md). Task P1-04 (part a).

## Context
The identity service can create firms, issue keys and authenticate a presented key, but nothing calls it yet. The gateway must be the only door: it authenticates tenant API keys, then routes to services. Services need to trust that a request came through the gateway and on whose behalf. ADR-0011 stated a limit: row-level security does not stop a *compromised service* from choosing any tenant, and named the gateway's signed tenant claim as the answer. This ADR defines that claim.

## Decision (proposed)
1. **Clients never reach services.** Services listen on a private interface only. The gateway is the single public entry.
2. **The gateway authenticates, services trust a signed token.** The gateway calls `identity.authenticate` for each tenant request, then forwards the call with a **30-second, EdDSA-signed JWT** (the internal token). The gateway holds the private key; every service holds only the public key(s).
3. **Three kinds of caller**, carried in the token's `kind` claim: `service` (the gateway itself, for internal calls), `operator` (platform operators, for creating firms) and `principal` (a tenant user or machine; carries tenant id, user id, key id and effective permissions). Each route declares which kinds it accepts.
4. **The tenant comes only from a verified token**, never from a header, path or body a client controls. The gateway strips `Authorization`, `X-TimeLex-*` and hop-by-hop headers before forwarding.
5. **Operator credential** (Phase 1): one or more SHA-256 hashes in configuration; the bearer is compared in constant time. No operator accounts yet.
6. **No authentication cache in the gateway (v1).** Every request calls `authenticate`, so a revoked key or a removed role stops working at once, as the identity contract promises. Revisit with measurements; if needed, a cache of at most a few seconds is an explicit trade against immediate revocation.
7. **Routing by path prefix** from a static table in configuration: `/v1/tenants` and `/v1/api-keys` go to `identity`. `/v1/authenticate` is **not** routable from outside. Header-based MCP routing, per-tenant rate limits and metering are P1-04 part b.
8. **Failures look alike.** Missing, malformed, unknown, revoked and expired credentials all return the same 401 body with `WWW-Authenticate: Bearer`. An unreachable upstream is a 502, a timeout a 504, and the gateway itself never forwards when authentication could not be completed.
9. **Limits:** request bodies up to 64 KiB (413 above), upstream timeout 5 seconds.

## Alternatives considered
- *Shared HMAC secret between gateway and services.* Simpler (no asymmetric keys) but every service can forge a token for any tenant, which defeats the point of ADR-0011's follow-up. Rejected as the default; acceptable only as a throwaway dev shortcut.
- *Forward the tenant API key and let each service call identity.* Spreads the secret across services, multiplies calls and logs, and lets any service see keys. Rejected.
- *Opaque tokens with introspection.* Equivalent to calling identity from every service. Rejected for the same reason.
- *Mutual TLS between services.* Strong transport identity, heavier to operate for one developer; it complements rather than replaces a claim about *whose request* this is. Revisit for the hosted offering.
- *A bespoke signed envelope instead of JWT.* Fewer dependencies, but a bespoke format is more to review. Standard JWT with a fixed algorithm and strict validation was chosen.

## Consequences
- A compromised service can no longer mint a tenant identity; it can only replay tokens it has seen for up to 30 seconds. Replay protection beyond that, per-service audiences with separate keys and mTLS are later work (listed in the contract).
- Every authenticated request costs one extra internal call and database read. Measure before caching.
- Key distribution is manual in Phase 1 (environment variables); the vault takes this over (P1-05).
- The identity contract changes: its callers are now only the gateway, and its security scheme becomes the internal token.
- Clock skew matters (30-second tokens, 5-second tolerance); deployments need time sync.
