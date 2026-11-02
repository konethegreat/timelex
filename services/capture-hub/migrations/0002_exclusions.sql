-- Written by Kone Tshivhinda & Claude
-- P2-07: per-firm privacy exclusions.
-- Run by timelex_migrator (search_path = capture). Needs the bootstrap files first.
--
-- A firm can exclude a domain, a client or a matter (ADR-0023). The capture service reads the
-- domain rows before every write and drops an event whose participant is at an excluded domain,
-- inside the same tenant transaction. Client and matter rows are configuration for attribution
-- (P3); a raw signal names neither, so they do not filter capture yet.
--
-- Values are stored in their canonical form: domains lower-case with no trailing dot, client and
-- matter identifiers trimmed but case-preserving. The service normalises; the constraints below
-- make a hand-written row obey the same shape.

CREATE TABLE capture.exclusions (
  tenant_id  uuid        NOT NULL,
  kind       text        NOT NULL,
  value      text        NOT NULL,
  reason     text,
  created_by text        NOT NULL,             -- the user who configured it, us_ form
  created_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, kind, value),
  CONSTRAINT exclusions_kind CHECK (kind IN ('domain', 'client', 'matter')),
  CONSTRAINT exclusions_value_length CHECK (char_length(value) BETWEEN 1 AND 253),
  CONSTRAINT exclusions_domain_shape CHECK (
    kind <> 'domain'
    OR value ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$'
  ),
  CONSTRAINT exclusions_reason_length CHECK (reason IS NULL OR char_length(reason) BETWEEN 1 AND 500),
  CONSTRAINT exclusions_reason_plain CHECK (reason IS NULL OR reason !~ '[[:cntrl:]]'),
  CONSTRAINT exclusions_created_by CHECK (created_by ~ '^us_[0-9A-Za-z]{8,64}$')
);

CALL platform.apply_tenant_rls('capture.exclusions');

-- An exclusion can be added and removed; it is never edited in place. Removing it and adding it
-- again records a new decision and a new time.
GRANT SELECT, INSERT, DELETE ON capture.exclusions TO svc_capture;
