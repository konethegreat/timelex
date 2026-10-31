-- Written by Kone Tshivhinda & Claude
-- P1-06: the platform outbox (ADR-0014). One queue for every service.
--
-- It lives in platform rather than in a service schema because the relay reads across firms, every
-- service table is FORCE row-level security, and no runtime role may bypass it. The queue holds only
-- the tenant id and the event the producer just wrote; a secret value never appears here.
--
-- A producer inserts and reads; it may move only the delivery columns. That stops a service from
-- rewriting an event it has already published.
CREATE TABLE IF NOT EXISTS platform.outbox (
  seq             bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  id              uuid        NOT NULL UNIQUE,
  tenant_id       uuid        NOT NULL,
  event_type      text        NOT NULL,
  payload         jsonb       NOT NULL,
  created_at      timestamptz NOT NULL DEFAULT now(),
  published_at    timestamptz,
  attempts        integer     NOT NULL DEFAULT 0,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  last_error      text,
  CONSTRAINT outbox_attempts_nonnegative CHECK (attempts >= 0)
);

-- The relay claims rows whose time has come, oldest first. Partial, because published rows are
-- never scanned again.
CREATE INDEX IF NOT EXISTS outbox_pending ON platform.outbox (next_attempt_at, seq)
  WHERE published_at IS NULL;
