-- Written by Kone Tshivhinda & Claude
-- P1-03: identity schema: tenants, users, roles, API keys.
-- Run by timelex_migrator (search_path = identity). Needs the three bootstrap files first.
--
-- Rules about this data live in Rust (crates/timelex-identity). The constraints below are a second
-- line of defence: they stop bad rows even if the Rust code has a bug.
-- Deliberately absent: passwords (people will sign in through their firm's Microsoft or Google
-- account later; only machines use API keys), and DELETE rights (see the grants at the end).

-- A firm.
CREATE TABLE identity.tenants (
  id           uuid        PRIMARY KEY,                      -- chosen by the Rust code, not the database
  slug         text        NOT NULL UNIQUE,                  -- short public name, e.g. "example-attorneys"
  display_name text        NOT NULL,
  status       text        NOT NULL DEFAULT 'active',
  created_at   timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT tenants_slug_format CHECK (slug ~ '^[a-z0-9][a-z0-9-]{1,38}[a-z0-9]$'),
  CONSTRAINT tenants_name_length CHECK (char_length(display_name) BETWEEN 1 AND 200),
  CONSTRAINT tenants_status      CHECK (status IN ('active', 'suspended'))
);

-- A person or a machine (a machine is a user with the 'agent' role).
-- The primary key starts with tenant_id so other tables can point at (tenant_id, id): a row can then
-- never reference a user from a different firm.
CREATE TABLE identity.users (
  tenant_id    uuid        NOT NULL REFERENCES identity.tenants (id),
  id           uuid        NOT NULL,
  email        text        NOT NULL,
  display_name text        NOT NULL,
  status       text        NOT NULL DEFAULT 'invited',
  created_at   timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, id),
  UNIQUE (tenant_id, email),
  CONSTRAINT users_email_normalised CHECK (
    email = lower(btrim(email))
    AND char_length(email) <= 254
    AND email ~ '^[^@[:space:]]+@[^@[:space:]]+\.[^@[:space:]]+$'),
  CONSTRAINT users_name_length CHECK (char_length(display_name) BETWEEN 1 AND 200),
  CONSTRAINT users_status      CHECK (status IN ('invited', 'active', 'disabled'))
);

-- Which roles a user holds. A user may hold several (a partner can be fee earner and billing).
CREATE TABLE identity.user_roles (
  tenant_id  uuid        NOT NULL,
  user_id    uuid        NOT NULL,
  role       text        NOT NULL,
  granted_by uuid        NOT NULL,          -- the first admin of a firm is granted by themselves
  granted_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, user_id, role),
  FOREIGN KEY (tenant_id, user_id)    REFERENCES identity.users (tenant_id, id),
  FOREIGN KEY (tenant_id, granted_by) REFERENCES identity.users (tenant_id, id),
  CONSTRAINT user_roles_role CHECK (role IN ('admin', 'fee_earner', 'billing', 'auditor', 'agent'))
);

-- API keys. The secret itself is NEVER stored: only its SHA-256 hash.
-- A token looks like  tlx_<tenant id as 32 hex characters>.<43 characters of random secret>
-- so the service can learn which firm to look in before it has authenticated anything
-- (the tenant part is a claim, not proof: the hash must match inside that firm).
CREATE TABLE identity.api_keys (
  tenant_id    uuid        NOT NULL,
  id           uuid        NOT NULL,
  user_id      uuid        NOT NULL,          -- the key acts as this user
  name         text        NOT NULL,          -- label chosen by the admin, e.g. "capture connector"
  key_prefix   text        NOT NULL,          -- first 8 characters of the secret part, shown in lists
  key_hash     bytea       NOT NULL UNIQUE,   -- SHA-256 of the whole token; this is the lookup index
  scopes       text[]      NOT NULL,          -- what the key may do (vocabulary lives in Rust)
  created_by   uuid        NOT NULL,
  created_at   timestamptz NOT NULL DEFAULT now(),
  expires_at   timestamptz,                   -- NULL = does not expire
  last_used_at timestamptz,
  revoked_at   timestamptz,
  revoked_by   uuid,
  PRIMARY KEY (tenant_id, id),
  FOREIGN KEY (tenant_id, user_id)    REFERENCES identity.users (tenant_id, id),
  FOREIGN KEY (tenant_id, created_by) REFERENCES identity.users (tenant_id, id),
  FOREIGN KEY (tenant_id, revoked_by) REFERENCES identity.users (tenant_id, id),
  CONSTRAINT api_keys_hash_length CHECK (octet_length(key_hash) = 32),
  CONSTRAINT api_keys_prefix      CHECK (key_prefix ~ '^[A-Za-z0-9_-]{8}$'),
  CONSTRAINT api_keys_name_length CHECK (char_length(name) BETWEEN 1 AND 100),
  CONSTRAINT api_keys_has_scopes  CHECK (cardinality(scopes) >= 1),
  CONSTRAINT api_keys_expiry      CHECK (expires_at IS NULL OR expires_at > created_at),
  CONSTRAINT api_keys_revocation  CHECK ((revoked_at IS NULL) = (revoked_by IS NULL))
);

-- Tenancy (P1-02): every table gets the standard policy. 'tenants' is keyed by its own id.
CALL platform.apply_tenant_rls('identity.tenants', 'id');
CALL platform.apply_tenant_rls('identity.users');
CALL platform.apply_tenant_rls('identity.user_roles');
CALL platform.apply_tenant_rls('identity.api_keys');

-- Privileges for the identity service role, spelled out column by column where it matters.
-- No DELETE anywhere except role removal: people and firms are disabled or suspended, never erased
-- (records may need to be kept; offboarding a firm is a separate, human-run procedure).
GRANT SELECT, INSERT ON identity.tenants, identity.users, identity.user_roles, identity.api_keys
  TO svc_identity;
GRANT UPDATE (display_name, status) ON identity.tenants TO svc_identity;
GRANT UPDATE (display_name, status) ON identity.users   TO svc_identity;
GRANT DELETE ON identity.user_roles TO svc_identity;
-- A key's hash, scopes, owner and expiry can never change. Rotation means a new key plus a revocation.
GRANT UPDATE (last_used_at, revoked_at, revoked_by) ON identity.api_keys TO svc_identity;
