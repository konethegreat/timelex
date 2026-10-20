# Internal service authentication (draft)

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · Status: **proposed**, see [ADR-0012](../docs/adr/0012-internal-service-authentication.md)_

How a TimeLex service knows that a request really came through the gateway, and on whose behalf. Services never see a tenant's API key and never accept one directly: the gateway checks the key and passes on a short-lived, signed **internal token**.

## The token
A JWT ([RFC 7519](https://www.rfc-editor.org/rfc/rfc7519)) signed with **EdDSA / Ed25519** ([RFC 8037](https://www.rfc-editor.org/rfc/rfc8037)), sent as `Authorization: Bearer <jwt>` on the gateway-to-service hop.

| Header field | Value |
|---|---|
| `alg` | `EdDSA` (the only accepted value; `none` and every other algorithm are rejected) |
| `typ` | `JWT` |
| `kid` | Id of the signing key, so keys can rotate (a service may trust two public keys at once) |

| Claim | Meaning |
|---|---|
| `iss` | `timelex-gateway` (the only issuer) |
| `aud` | The service the call is for, for example `identity`. A service rejects tokens addressed to another service |
| `iat`, `exp` | Issued-at and expiry. Lifetime **30 seconds**; services allow **5 seconds** of clock skew |
| `jti` | Unique id per token (for the audit log; not used for replay blocking in v1) |
| `kind` | Who is calling: `service` (the gateway itself, for internal calls such as `authenticate`), `operator` (a platform operator, for `createTenant`), or `principal` (a tenant user or machine acting through an API key) |
| `tid` | Tenant id. **Present only when `kind` is `principal`** |
| `sub` | User id. Only for `principal` |
| `key` | API-key id. Only for `principal` |
| `perms` | The principal's effective permissions, as returned by `authenticate`. Only for `principal` |

Validation, in order, and every failure is a 401 with the same body: signature against a trusted public key for `kid` → `alg` is `EdDSA` → `iss` → `aud` equals this service → `exp` not passed (±5 s) → `iat` not in the future (±5 s) → `kind` is allowed for this route → for `principal`, `tid`, `sub`, `key` and `perms` are present and well-formed.

## Why signatures, not a shared secret
The gateway holds the **private** key; each service holds only the **public** key. A compromised service can therefore read tokens but cannot mint one for another service or for another tenant. That closes the gap that [ADR-0011](../docs/adr/0011-tenancy-enforcement-and-database-roles.md) states plainly: row-level security alone does not stop a compromised service from choosing any tenant.

## What the gateway does with a request
1. Reads `Authorization: Bearer ...`. Anything else is a 401.
2. For routes that need a tenant user: calls `identity.authenticate` with a `service` token; on success builds a `principal` token and forwards the request.
3. For operator routes: compares the bearer against the configured operator credential(s) (SHA-256, constant-time) and forwards with an `operator` token.
4. **Strips** any client-supplied `Authorization`, `X-TimeLex-*` and hop-by-hop headers before forwarding, so a client cannot smuggle an identity.
5. Never forwards the original API key.

## Keys and configuration (Phase 1)
| What | Where |
|---|---|
| Gateway private key (Ed25519, PKCS#8, base64) | Environment variable `TIMELEX_GATEWAY_SIGNING_KEY`, later the vault. Never in the repository |
| Gateway public key(s), by `kid` | Environment variable on every service, `TIMELEX_GATEWAY_PUBLIC_KEYS` (`kid=base64,kid=base64`) |
| Operator credential(s) | Environment variable `TIMELEX_OPERATOR_KEY_SHA256` (hex SHA-256 of the key; comma-separated for rotation) |
| Dev key generation | A subcommand of the gateway binary prints a fresh key pair; nothing is committed |

## Not covered yet
Replay prevention beyond the 30-second lifetime, mutual TLS between services, per-service keys, token binding to the request path. Services must listen on a private interface only; the token is a second line of defence, not a substitute for network isolation.
