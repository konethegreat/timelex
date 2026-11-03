-- Written by Kone Tshivhinda & Claude
-- P3-04: minimal matters: clients, matters, aliases and rate cards (ADR-0027).
-- Run by timelex_migrator with search_path = matters; the platform bootstrap must have provisioned
-- the service first.
--
-- The firm's own keys. A client and a matter are named by the identifier the practice system gives
-- them (a `clientId` and a `matterId`), never by a surrogate UUID we invent. An import can round-trip,
-- and attribution can store the same key the labels use. Every table is tenant-scoped; the
-- constraints repeat the rules in src/domain.rs as a second line of defence.

-- A client of the firm.
CREATE TABLE matters.clients (
  tenant_id  uuid        NOT NULL,
  client_id  text        NOT NULL,
  name       text        NOT NULL,
  status     text        NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, client_id),
  CONSTRAINT clients_id_length   CHECK (char_length(client_id) BETWEEN 1 AND 64),
  CONSTRAINT clients_id_chars    CHECK (client_id ~ '^[A-Za-z0-9._:-]+$'),
  CONSTRAINT clients_name_length CHECK (char_length(name) BETWEEN 1 AND 200),
  CONSTRAINT clients_name_line   CHECK (name !~ '[[:cntrl:]]'),
  CONSTRAINT clients_status      CHECK (status IN ('active', 'inactive'))
);

-- A matter under a client.
CREATE TABLE matters.matters (
  tenant_id     uuid        NOT NULL,
  matter_id     text        NOT NULL,
  client_id     text        NOT NULL,
  reference     text        NOT NULL,             -- the firm's spelling, shown to people
  reference_key text        NOT NULL,             -- case-folded, for uniqueness and matching
  title         text        NOT NULL,
  status        text        NOT NULL DEFAULT 'open',
  practice_area text,
  opened_on     date,
  closed_on     date,
  created_at    timestamptz NOT NULL DEFAULT now(),
  updated_at    timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, matter_id),
  UNIQUE (tenant_id, reference_key),
  UNIQUE (tenant_id, matter_id, client_id),       -- lets an alias prove the matter is this client's
  FOREIGN KEY (tenant_id, client_id) REFERENCES matters.clients (tenant_id, client_id),
  CONSTRAINT matters_id_length        CHECK (char_length(matter_id) BETWEEN 1 AND 64),
  CONSTRAINT matters_id_chars         CHECK (matter_id ~ '^[A-Za-z0-9._:-]+$'),
  CONSTRAINT matters_reference_length CHECK (char_length(reference) BETWEEN 1 AND 64),
  CONSTRAINT matters_reference_line   CHECK (reference !~ '[[:cntrl:]]'),
  CONSTRAINT matters_reference_key    CHECK (reference_key = lower(btrim(reference))),
  CONSTRAINT matters_title_length     CHECK (char_length(title) BETWEEN 1 AND 200),
  CONSTRAINT matters_title_line       CHECK (title !~ '[[:cntrl:]]'),
  CONSTRAINT matters_status           CHECK (status IN ('open', 'closed')),
  CONSTRAINT matters_practice_area    CHECK (
    practice_area IS NULL
    OR (char_length(practice_area) BETWEEN 1 AND 100 AND practice_area !~ '[[:cntrl:]]')),
  CONSTRAINT matters_dates_order      CHECK (closed_on IS NULL OR opened_on IS NULL OR closed_on >= opened_on)
);

-- What a signal may name instead of the canonical key: a reference seen in a document, a client's
-- display name, or an email domain. Values are stored case-folded; the same value can name only one
-- target per firm.
CREATE TABLE matters.aliases (
  tenant_id  uuid        NOT NULL,
  id         uuid        NOT NULL,               -- surrogate only; the value is the key
  kind       text        NOT NULL,
  value      text        NOT NULL,
  client_id  text        NOT NULL,
  matter_id  text,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, id),
  UNIQUE (tenant_id, kind, value),
  FOREIGN KEY (tenant_id, client_id) REFERENCES matters.clients (tenant_id, client_id),
  -- MATCH SIMPLE: when matter_id is NULL the whole foreign key is not checked.
  FOREIGN KEY (tenant_id, matter_id, client_id)
    REFERENCES matters.matters (tenant_id, matter_id, client_id),
  CONSTRAINT aliases_kind           CHECK (kind IN ('matter_reference', 'client_name', 'client_domain')),
  CONSTRAINT aliases_value_length   CHECK (char_length(value) BETWEEN 1 AND 320),
  CONSTRAINT aliases_value_canonical CHECK (value = lower(btrim(value))),
  CONSTRAINT aliases_matter_target  CHECK ((kind = 'matter_reference') = (matter_id IS NOT NULL))
);

-- One rate card per matter. P5-01 adds discounts, arrangements and firm-wide defaults; P3-04 stores
-- only the hourly rate and the unit policy a time entry will be billed by.
CREATE TABLE matters.rate_cards (
  tenant_id          uuid        NOT NULL,
  matter_id          text        NOT NULL,
  hourly_rate_cents  bigint      NOT NULL,
  unit_minutes       smallint    NOT NULL DEFAULT 6,
  rounding           text        NOT NULL DEFAULT 'up',
  minimum_units      smallint    NOT NULL DEFAULT 1,
  updated_at         timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, matter_id),
  FOREIGN KEY (tenant_id, matter_id) REFERENCES matters.matters (tenant_id, matter_id),
  CONSTRAINT rate_cards_hourly  CHECK (hourly_rate_cents BETWEEN 0 AND 100000000),
  CONSTRAINT rate_cards_unit    CHECK (unit_minutes BETWEEN 1 AND 60),
  CONSTRAINT rate_cards_minimum CHECK (minimum_units BETWEEN 0 AND 60),
  CONSTRAINT rate_cards_rounding CHECK (rounding IN ('up', 'down', 'nearest'))
);

-- Tenancy (ADR-0011): the standard policy on every table.
CALL platform.apply_tenant_rls('matters.clients');
CALL platform.apply_tenant_rls('matters.matters');
CALL platform.apply_tenant_rls('matters.aliases');
CALL platform.apply_tenant_rls('matters.rate_cards');

-- Privileges for the service role. No DELETE: clients are made inactive and matters are closed, so
-- history that attribution and billing refer to cannot vanish.
GRANT SELECT, INSERT, UPDATE ON matters.clients, matters.matters, matters.aliases, matters.rate_cards
  TO svc_matters;
