# ADR-0024: Disclosure is a versioned register, served as it is

- **Status:** Accepted
- **Date:** 2026-11-02
- **Task:** P2-10 (consent and disclosure UX for admins and users)
- **Relates to:** ADR-0007 (tenancy and data residency, point 7), ADR-0012 (gateway routing), ADR-0022 (manual quick capture), ADR-0023 (privacy exclusions)

## Context

ADR-0007 point 7 requires every tenant to have an exportable record of what data goes to which processor
for the firm's POPIA documentation. The record has to be useful to two different readers: a firm's
administrator, who needs to understand what this build records and can change, and the firm's compliance
file, which needs a stable artefact it can attach to a POPIA register and compare when the product
changes.

Two shortcuts would undermine that. A generated, per-tenant document invites the claim that the service
tailors its data flows per firm, which is not true in this phase: the flows are properties of the build,
not of the tenant. A document that lists only what is switched on invites the opposite error: a reader
cannot tell whether a missing processor is absent or simply unlisted.

The disclosure also has to be honest about consent. An exclusion added today changes nothing that was
already stored (ADR-0023), and this build has no automatic retention or deletion. If the register implies
otherwise, the firm's notice to its clients would be wrong.

## Decision

### 1. One static, versioned register describes the build

The register is a JSON file in `contracts/privacy/data-flow.v1.json`, reviewed and committed with the
code, not generated per tenant. It carries a `version` and an `updatedAt`. Every consumer reads the same
bytes. A change to a data flow, a storage location, a processor or a default is a code change with a
version bump, visible in review and in git history.

### 2. It is served publicly at `GET /v1/privacy/data-flow`

The route is open, uses no tenant credential and sets `no-store`, so an administrator can download the
current register without a key. The register describes the platform and contains no tenant data. The
gateway embeds the same file it serves to the admin page, so the page and the export cannot drift.

### 3. The register states what is inactive as well as what is active

Every flow and processor carries a status of `active`, `development_only` or `not_active`. Mail and
calendar connectors, model inference, Anthropic, Microsoft Graph and Google Workspace are listed as
`not_active` rather than omitted, and the content category says that no content store is wired. A
reader can therefore distinguish "not yet built" from "not disclosed".

### 4. Facts, not promises, and explicitly not legal advice

The register records the defaults a reader can check against the code: content is not captured by
default, redaction is required before content, manual entries store only the title, and domain
exclusions are enforced before storage. It also records that an exclusion does not apply to existing
events and that no automatic retention is configured. It carries a disclaimer that it is a technical
record, not legal advice, and that it does not replace the firm's own records or its counsel's review.

### 5. The admin page is self-contained and stateless

`GET /privacy` serves one HTML file from the gateway, in the same pattern as the quick-capture page: a
strict CSP, `nosniff`, `no-referrer`, `no-store` and no third-party asset. It renders the register and
lets an administrator list, add and remove exclusions. The API key is held in page memory only, never in
a cookie, URL or web storage, and a reload asks for it again. Exclusion changes go through the existing
`/v1/privacy/exclusions` routes, so the page has no privileged path of its own and capture still
enforces `settings:manage`.

## Alternatives considered

- **Generate a register per tenant.** It could include a firm's own processor choices and region policy,
  but those settings do not exist yet, and generating a document would make staleness easy while
  implying per-firm tailoring the build cannot honour. A static register plus the firm's own file is the
  honest version until per-firm policy exists.
- **Publish only in `docs/`.** Easy to write, hard to export from a running deployment and easy to drift
  from the code. Serving the committed file gives the compliance file a stable URL and a version.
- **A PDF export.** A formatting pass over the same facts, with a second place to drift. JSON is the
  source; a firm can convert it for its file, and a human-readable page already exists at `/privacy`.
- **Put the key in `sessionStorage` so a reload keeps it.** Survives a reload but is readable by any
  script on the origin, and a reload is not worth that. The quick-capture page made the same choice
  after CodeQL flagged the storage version (P2-08).

## Consequences

- When the first connector ships, its flow moves from `not_active` to `active`, the register version is
  bumped and the change is reviewed like any other contract.
- Per-firm processor choices, retention rules and the region policy switch will need a per-tenant layer;
  point 1's static register remains the platform baseline it extends.
- The page is not a consent record. It discloses defaults and manages exclusions; a signed, per-user
  consent artefact remains open work under P2-10.
- The register is a contract-shaped file and is covered by a gateway test asserting its version, its
  honest defaults, its exclusion scope and the absence of credential-shaped strings.
