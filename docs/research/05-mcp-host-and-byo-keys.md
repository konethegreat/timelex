# 05 · MCP host design and bring-your-own-key economics

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

## 1. Terms (so we stop mixing them up)
| Term | Meaning in MCP | In TimeLex |
|---|---|---|
| **MCP server** | Exposes tools, resources and prompts | Every service exposes one (`/mcp`), plus REST |
| **MCP client** | Protocol connector, one per server connection | Lives inside `mcp-host` |
| **MCP host** | The application that owns the LLM loop, manages clients, enforces consent and policy | `services/mcp-host`: a **server-side** host running agent loops for tenants |
| **Outer agent** | A user's own Claude / plugin / IDE | Connects *to* our gateway as a client of our MCP servers |

So "an MCP host backend" has two faces: **inward** (our agents call our services and third-party MCP servers) and **outward** (customers' agents call us).

## 2. Where the protocol is (verified)
- The **2026-07-28 specification is final**; the release post is dated 28 July 2026. Tier-1 SDKs (TypeScript, Python, Go, C#) support it; **the Rust SDK has beta support** ([MCP blog](https://blog.modelcontextprotocol.io/posts/2026-07-28/)).
- **Stateless core:** no `initialize` handshake, no `Mcp-Session-Id`; each request carries client info and capabilities in `_meta`; optional `server/discover`; any instance can serve any request, so plain round-robin load balancing works.
- **Multi Round-Trip Requests:** a server can ask the client for input mid-call (the shape of human approval) without holding a stream open.
- **Header routing** (`Mcp-Method`, `Mcp-Name`) lets gateways route without parsing bodies.
- **Extensions:** **Tasks** (poll/update/cancel for long jobs), **MCP Apps** (sandboxed UI), **Enterprise Managed Authorization**.
- **Auth:** OAuth 2.1/OIDC alignment, RFC 9207 issuer validation; Dynamic Client Registration deprecated in favour of Client ID Metadata Documents.
- **Deprecated (≥ 12 months notice):** **Roots, Sampling, Logging**, and the legacy HTTP+SSE transport ([AAIF migration guide](https://aaif.io/blog/mcp-2026-07-28-whats-changing-and-how-to-migrate)). Earliest removal 28 July 2027.
- **ClearCut found** that `rmcp` 3.5.1 still spoke protocol `2025-06-18` as of 19 Oct 2026 ([ClearCut CLAUDE.md](https://github.com/konethegreat/clearcut/blob/main/CLAUDE.md)). "Beta support" and "released crate" are different things. **Task P1-08 verifies the current Rust SDK before we commit to it**, with a fallback of a thin hand-written Streamable-HTTP adapter over the stateless spec.

## 3. Design consequences
1. **Stateless servers.** State lives in Postgres and moves through explicit handles (`task_id`, `entry_id`, `batch_id`). This matches the microservice goal exactly.
2. **Approvals use Multi Round-Trip Requests.** Agent drafts, server returns `InputRequiredResult` with a `requestState`, a human (or policy) answers, the client retries.
3. **Long jobs use Tasks** (back-filling a month of mail, re-attributing a matter, generating a pro-forma run).
4. **No Sampling.** Our host calls the model API directly with the tenant's key; it does not ask clients to sample. That also keeps the key custody story simple.
5. **Header-based routing at the gateway** for per-tenant rate limits and metering without body parsing.
6. **MCP Apps** for the review queue inside Claude clients later (stretch).

## 4. Host responsibilities (what `mcp-host` must do)
| Responsibility | Mechanism |
|---|---|
| Tenant isolation | Every call carries tenant ID; tools resolve data through the tenant's schema/role only |
| Model access | Provider trait (Anthropic first); keys fetched from the vault per call; never logged or placed in prompts |
| Tool governance | Per-workflow **allow-lists**; annotations distinguish read-only from mutating; mutating tools require idempotency keys |
| Budgets | Per-tenant, per-workflow and per-job token and Rand caps; abort on breach |
| Escalation ladder | Haiku → Sonnet → Opus → Fable → human, driven by validation and disagreement, configurable per tenant |
| Context packs | Everything a task needs and nothing else, logged per call (same principle as ClearCut ADR-0005) |
| Audit | Actor, tool, inputs, outputs, policy decision, model, tokens, cost |
| Safety | Untrusted-content separation; no outbound network tools in extraction workflows; memory writes only from human-confirmed corrections |
| Outward MCP | Aggregates service MCP tools behind the gateway with the same authz |

## 5. Bring-your-own-key (BYO) model
**What the customer brings**
- **Anthropic API key** (their Console workspace, their billing, their usage limits).
- **Connector credentials:** Microsoft Entra app consent (admin consent) or their own app registration; Google OAuth; optional Zoom/Twilio/others.
- **Optional:** their own model endpoint behind the provider trait later (in-region/self-hosted, for firms whose counsel will not allow US processing).

**What we never do:** route a tenant's requests through a key that is not theirs; pool keys; keep plaintext keys after write; return a key from any API (write-only, with a last-four fingerprint and a "test key" call).

**Why it is the right commercial shape**
- AI cost is variable and, per [research 04](04-autonomous-capture-design.md) §7, small (illustratively **R13–R56 per fee earner per month**) compared with competitor licences (**R700–R3 000+**). Letting the firm pay the provider directly keeps our margin in the platform and makes the cost auditable by the customer.
- It removes our largest variable cost and the incentive to downgrade models to save margin.
- It aligns with Anthropic's usage policy by keeping each firm in its own agreement.

**What it costs us**
- *Quality variance:* firms may pick weak models or revoke keys. Mitigation: recommended policy packs per tier, eval harness, health checks that downgrade autonomy automatically.
- *Support burden:* "my key stopped working" is the new "the server is down". Mitigation: key test on save, spend alerts, usage dashboard with provider-reported costs.
- *Spend risk:* an agent loop could burn the customer's money. Mitigation: hard Rand caps in `mcp-host` plus Anthropic workspace spend limits (recommended in onboarding).

## 6. Key custody (summary; full design in [ADR-0004](../adr/0004-byo-keys-and-secrets-vault.md))
- Envelope encryption: AES-256-GCM data keys per secret, wrapped by a key-encryption key in a cloud KMS (a South African region where available, for residency) or a local sealed key for self-hosters.
- Decryption only inside `mcp-host`/`capture-hub` workers, in memory, for the duration of a call.
- Rotation, revocation, access log, tamper-evident audit.
- Separate environments never share keys. Keys are never in env files committed to git, in logs, in traces or in prompts.

## 7. Open questions
| ID | Question |
|---|---|
| P1-08 | Is `rmcp` (or another Rust SDK) on the final 2026-07-28 spec yet? |
| T0-L02 | Does Anthropic's agreement permit a platform storing a customer's key on the customer's behalf in this way, and what do they recommend (e.g. workspace-scoped keys)? |
| T0-A01 | Should Anthropic's **Managed Agents** or the Agent SDK be used instead of our own loop? Trade-off: speed to build versus control of keys, policy, residency and multi-provider. Default: our loop, revisit at Phase 4 |
| T0-A02 | Provider-reported cost APIs: can we reconcile our metering against the customer's actual bill? |

## Sources
[MCP 2026-07-28 release](https://blog.modelcontextprotocol.io/posts/2026-07-28/) · [AAIF migration notes](https://aaif.io/blog/mcp-2026-07-28-whats-changing-and-how-to-migrate) · [MCP architecture spec](https://modelcontextprotocol.io/specification/2026-07-28/architecture) · [Anthropic Usage Policy](https://www.anthropic.com/legal/aup) · [ClearCut research 03](https://github.com/konethegreat/clearcut/blob/main/docs/research/03-mcp-and-agentic.md)
