-- Written by Kone Tshivhinda & Claude
-- P2-02: the capture schema: normalised activity events and per-connector cursors.
-- Run by timelex_migrator (search_path = capture). Needs the bootstrap files first.
--
-- An activity event is a fact about work that happened (ADR-0006). The table is append-only: no
-- UPDATE or DELETE is granted, so a stored event cannot be edited. A correction is a new event.
-- The id is derived from the signal (contracts/events/README.md), and the primary key makes a
-- duplicate delivery land on the existing row instead of creating a second event.
--
-- The full contract envelope is kept in `event` so downstream services can read the event as the
-- contract defines it, while the columns beside it carry the facts that are queried and constrained.

CREATE TABLE capture.activity_events (
  tenant_id     uuid        NOT NULL,
  id            text        NOT NULL,             -- ae_ + 32 lowercase hex, derived from the signal
  source_system text        NOT NULL,             -- the contract's source.system spelling
  source_id     text        NOT NULL,             -- the id in the originating system
  actor_id      text        NOT NULL,             -- the user the signal belongs to, us_ form
  kind          text        NOT NULL,             -- what kind of work it was
  started_at    timestamptz NOT NULL,
  ended_at      timestamptz NOT NULL,
  captured_at   timestamptz NOT NULL,
  recorded_at   timestamptz NOT NULL,             -- when capture-hub accepted the event
  event         jsonb       NOT NULL,             -- the complete ActivityEvent envelope
  PRIMARY KEY (tenant_id, id),
  -- The same signal can only be one event, even if a producer computes a different id. The
  -- fingerprint is the four facts the id rule uses, not the id it produced.
  CONSTRAINT activity_events_signal_unique
    UNIQUE (tenant_id, source_system, source_id, started_at),
  CONSTRAINT activity_events_id_format CHECK (id ~ '^ae_[0-9a-f]{32}$'),
  CONSTRAINT activity_events_source_system CHECK (source_system IN (
    'm365_mail', 'm365_calendar', 'm365_teams', 'google_mail', 'google_calendar',
    'desktop_agent', 'manual', 'import')),
  CONSTRAINT activity_events_kind CHECK (kind IN (
    'email_sent', 'email_received', 'meeting', 'call', 'document_edit', 'document_view',
    'web_research', 'desktop_focus', 'manual_entry')),
  CONSTRAINT activity_events_actor_format CHECK (actor_id ~ '^us_[0-9A-Za-z]{8,64}$'),
  CONSTRAINT activity_events_source_id_length CHECK (char_length(source_id) BETWEEN 1 AND 512),
  CONSTRAINT activity_events_time_order CHECK (ended_at >= started_at),
  CONSTRAINT activity_events_payload_size CHECK (octet_length(event::text) <= 65536)
);

-- A connector stores where it got to. Advancing is a single upsert, so a replay from an earlier
-- cursor is the only way to go backwards, and it is explicit.
CREATE TABLE capture.connector_cursors (
  tenant_id  uuid        NOT NULL,
  connector  text        NOT NULL,               -- the connector's fixed name, e.g. m365_mail
  cursor     text        NOT NULL,               -- opaque to the platform; interpreted by the connector
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, connector),
  CONSTRAINT connector_cursors_name CHECK (connector ~ '^[a-z][a-z0-9_]{0,63}$'),
  CONSTRAINT connector_cursors_value CHECK (char_length(cursor) BETWEEN 1 AND 4096)
);

CALL platform.apply_tenant_rls('capture.activity_events');
CALL platform.apply_tenant_rls('capture.connector_cursors');

-- Privileges for the capture service role. An event is inserted once and never changed: no UPDATE,
-- no DELETE. A cursor advances in place, which needs UPDATE.
GRANT SELECT, INSERT ON capture.activity_events TO svc_capture;
GRANT SELECT, INSERT, UPDATE ON capture.connector_cursors TO svc_capture;
