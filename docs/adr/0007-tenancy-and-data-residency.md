# ADR-0007: Multi-tenancy and data residency

Status: Proposed · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude

## Context
Law-firm data is confidential and personal. Hosted use needs strong tenant isolation; some firms will self-host. POPIA section 72 governs cross-border transfers, and provider inference may occur outside South Africa ([research 03](../research/03-south-africa-compliance.md)).

## Decision
1. **Self-host first-class.** Everything runs from Docker Compose / Helm with no mandatory TimeLex-operated dependency.
2. **Hosted deployments run in a South African region** where the cloud provider offers one, for data at rest. Model inference location is a separate, disclosed property of the provider configuration.
3. **Isolation model:** shared Postgres cluster, **row-level security keyed by `tenant_id`** in every service schema, with the tenant set per request by the platform kit and enforced by database policy (not only by application code). Tests assert cross-tenant reads fail.
4. **Option to upgrade** large or sensitive tenants to a **dedicated schema or database** later without API change (tenant ID stays in contracts).
5. **Per-tenant encryption keys** (see ADR-0004) so a tenant can be crypto-shredded.
6. **No tenant data in another tenant's model context**, no shared memory stores, no shared embeddings index (one index namespace per tenant).
7. **Data-flow register**: every tenant has an exportable record of what data goes to which processor (model provider, KMS, email provider) for the firm's POPIA documentation.
8. **Region and provider disclosure** in the admin UI, with a policy switch "block model calls unless provider region ∈ allowed set".

## Evidence update (20 Oct 2026; clarifies point 2 and 8, no decision change)
Anthropic offers **no South African or African inference region**: `inference_geo` is `global` or `us` only, and workspace geo is `us` only ([research 07 §4](../research/07-anthropic-terms-and-key-custody.md)). Point 8's policy switch "block model calls unless provider region is in the allowed set" therefore blocks **every** Anthropic call for a firm that allows only South African processing. The admin UI and sales material must say so plainly. A South African region for TimeLex's own data at rest does not change where inference runs. Also: rule 54.9.2.1 does not require South African hosting for offsite electronic records but does require confidentiality, security, immediate access for the firm's office and the Council, and a provider contract ([research 03 §1.2](../research/03-south-africa-compliance.md)).

## Alternatives considered
- *Schema-per-tenant* (as used in some SaaS backends): stronger isolation, heavier operations and migrations at scale. Keep as the upgrade path.
- *Database-per-tenant from day one:* highest isolation, highest cost; unnecessary for early pilots.
- *Application-level filtering only:* rejected; one missed `WHERE` leaks a firm's clients.

## Consequences
- RLS policies and tenant-context middleware are Phase 1 deliverables with property tests.
- Backups and restores need per-tenant procedures; LPC computerised-record rules require monthly off-site backups for ledgers (rule 54.14.3).
