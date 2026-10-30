-- Written by Kone Tshivhinda & Claude
-- P1-05: vault schema: secrets, the per-firm audit chain and its head.
-- Run by timelex_migrator (search_path = vault). Needs the bootstrap files first.
--
-- The rules about secrets live in Rust (crates/timelex-vault). The constraints below are a second
-- line of defence, as in identity: they stop a bad row even if the Rust code has a bug.
--
-- Nothing here can hold a plaintext secret. A value exists only as AES-256-GCM ciphertext whose data
-- key is itself wrapped by the key-encryption key, which is not in this database (ADR-0004).
-- Revoking a secret NULLs every sealed column: the ciphertext is destroyed, not just marked.

CREATE TABLE vault.secrets (
  tenant_id     uuid        NOT NULL,
  id            uuid        NOT NULL,           -- chosen by the Rust code
  kind          text        NOT NULL,           -- vocabulary lives in Rust; one live row per kind
  label         text        NOT NULL,           -- chosen by an admin, e.g. "Firm Claude key"
  last_four     text        NOT NULL,           -- the only part of a value ever shown
  allowed_hosts text[]      NOT NULL,           -- narrowed from the kind's provider hosts, never widened
  -- The sealed value. All five columns are set together and NULLed together on revocation.
  ciphertext    bytea,                          -- AES-256-GCM output: 16-byte tag + value
  value_nonce   bytea,                          -- 12 bytes, one per sealing
  wrapped_dek   bytea,                          -- data key encrypted by the key-encryption key: tag + 32
  dek_nonce     bytea,                          -- 12 bytes, one per sealing
  key_version   text,                           -- which key-encryption key wrapped the data key
  created_by    uuid        NOT NULL,
  created_at    timestamptz NOT NULL,
  updated_at    timestamptz NOT NULL,
  last_tested_at timestamptz,                   -- when a provider check last ran (pass or fail)
  last_used_at  timestamptz,                    -- reserved for leases (P1-13); never a value
  revoked_at    timestamptz,
  revoked_by    uuid,
  PRIMARY KEY (tenant_id, id),
  CONSTRAINT secrets_kind CHECK (
    kind IN ('anthropic_api_key', 'm365_oauth', 'google_oauth', 'generic_api_key')),
  CONSTRAINT secrets_label_length CHECK (char_length(label) BETWEEN 1 AND 120),
  CONSTRAINT secrets_last_four_format CHECK (last_four ~ '^[!-~]{4}$'),
  CONSTRAINT secrets_hosts_count CHECK (cardinality(allowed_hosts) BETWEEN 1 AND 16),
  -- Coarse second line: the full DNS rule (labels, dotted, no wildcard) is in Rust.
  CONSTRAINT secrets_hosts_format CHECK (
    allowed_hosts = array_remove(allowed_hosts, NULL)
    AND array_to_string(allowed_hosts, ',') ~ '^[a-z0-9.-]+(,[a-z0-9.-]+)*$'),
  CONSTRAINT secrets_sealed_together CHECK (
    (ciphertext IS NULL AND value_nonce IS NULL AND wrapped_dek IS NULL AND dek_nonce IS NULL
       AND key_version IS NULL)
    OR (ciphertext IS NOT NULL AND value_nonce IS NOT NULL AND wrapped_dek IS NOT NULL
       AND dek_nonce IS NOT NULL AND key_version IS NOT NULL)),
  CONSTRAINT secrets_sealed_lengths CHECK (
    ciphertext IS NULL
    OR (octet_length(ciphertext) BETWEEN 16 AND 4112
        AND octet_length(value_nonce) = 12
        AND octet_length(wrapped_dek) = 48
        AND octet_length(dek_nonce) = 12
        AND key_version ~ '^[a-z0-9]+$')),
  CONSTRAINT secrets_revocation CHECK ((revoked_at IS NULL) = (revoked_by IS NULL)),
  -- Revoking destroys the value; a revoked row keeps only metadata (including when it was last tested).
  CONSTRAINT secrets_revoked_has_no_value CHECK (revoked_at IS NULL OR ciphertext IS NULL)
);

-- One live secret per kind: PUT /v1/secrets/{kind} replaces. Revoked rows stay behind for history
-- and do not stand in the way of a new secret of the same kind.
CREATE UNIQUE INDEX secrets_one_live_kind
  ON vault.secrets (tenant_id, kind)
  WHERE revoked_at IS NULL;

-- The audit chain (ADR-0004 point 7). Append-only: no UPDATE or DELETE is ever granted.
-- Each entry holds the previous entry's hash and its own; rewriting history breaks every later hash.
CREATE TABLE vault.secret_audit (
  tenant_id  uuid   NOT NULL,
  seq        bigint NOT NULL,                   -- 1, 2, 3... per firm
  at         timestamptz NOT NULL,
  action     text   NOT NULL,
  actor_id   uuid   NOT NULL,                   -- the user whose request caused the action
  secret_id  uuid   NOT NULL,
  detail     text   NOT NULL DEFAULT '',        -- fixed vocabulary: kind, or ok/failed; never a value
  prev_hash  bytea  NOT NULL,                   -- 32 zero bytes for the first entry
  entry_hash bytea  NOT NULL,
  PRIMARY KEY (tenant_id, seq),
  CONSTRAINT secret_audit_action CHECK (action IN ('created', 'rotated', 'revoked', 'tested', 'used')),
  CONSTRAINT secret_audit_detail_length CHECK (char_length(detail) <= 120),
  CONSTRAINT secret_audit_prev_hash_length CHECK (octet_length(prev_hash) = 32),
  CONSTRAINT secret_audit_entry_hash_length CHECK (octet_length(entry_hash) = 32),
  -- Two entries cannot share a hash, which would mean two histories claiming the same step.
  CONSTRAINT secret_audit_hash_unique UNIQUE (tenant_id, entry_hash)
);

-- The last entry of each firm's chain. Row-locked while appending, so two writers cannot both claim
-- the same position.
CREATE TABLE vault.chain_heads (
  tenant_id uuid   PRIMARY KEY,
  seq       bigint NOT NULL,                    -- 0 before the first entry
  hash      bytea  NOT NULL,                    -- GENESIS (32 zero bytes) at seq 0
  CONSTRAINT chain_heads_hash_length CHECK (octet_length(hash) = 32)
);

CALL platform.apply_tenant_rls('vault.secrets');
CALL platform.apply_tenant_rls('vault.secret_audit');
CALL platform.apply_tenant_rls('vault.chain_heads');

-- Privileges for the vault service role, column by column.
-- No DELETE: a secret is revoked, never erased. A secret's identity, kind and creator never change;
-- revocation NULLs the sealed columns through the UPDATE grant.
GRANT SELECT, INSERT
  ON vault.secrets, vault.secret_audit TO svc_vault;
GRANT UPDATE (label, allowed_hosts, last_four, ciphertext, value_nonce, wrapped_dek, dek_nonce,
              key_version, updated_at, last_tested_at, last_used_at, revoked_at, revoked_by)
  ON vault.secrets TO svc_vault;
GRANT UPDATE (seq, hash) ON vault.chain_heads TO svc_vault;
GRANT SELECT, INSERT ON vault.chain_heads TO svc_vault;
