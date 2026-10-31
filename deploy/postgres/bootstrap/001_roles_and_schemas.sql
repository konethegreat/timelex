-- Written by Kone Tshivhinda & Claude
-- P1-01: one schema and one database role per service.
--
-- Run ONCE per database, as a role that may create roles (the superuser in local dev, the managed
-- admin user in a hosted database). Safe to re-run. Never put passwords in this file: login
-- passwords are set by the deployment (see deploy/postgres/README.md).

-- 1. The migrator: owns every service schema and runs each service's own migrations.
--    It is NOT a superuser and does NOT bypass row-level security.
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'timelex_migrator') THEN
    CREATE ROLE timelex_migrator NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
  END IF;
END
$$;

-- 2. Nobody gets anything by default. Everything a role can do is an explicit grant.
DO $$
BEGIN
  EXECUTE format('REVOKE ALL ON DATABASE %I FROM PUBLIC', current_database());
  EXECUTE format('GRANT CONNECT ON DATABASE %I TO timelex_migrator', current_database());
END
$$;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
-- Functions created by the migrator are executable only by roles we grant, not by everyone.
ALTER DEFAULT PRIVILEGES FOR ROLE timelex_migrator REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;

-- 3. A schema for platform-wide helpers (tenancy function, provisioning). It holds no tenant data.
CREATE SCHEMA IF NOT EXISTS platform;
REVOKE ALL ON SCHEMA platform FROM PUBLIC;
GRANT USAGE ON SCHEMA platform TO timelex_migrator;

-- 4. The one way to add a service. It creates (if missing):
--      role   svc_<service>   NOLOGIN, no special powers, search_path = its own schema
--      schema <service>       owned by timelex_migrator; only svc_<service> may even look inside
--    It does NOT create tables. Each service's own migrations do that and grant table privileges
--    explicitly. Service names are restricted so they can never collide with system schemas.
--    Needs platform.grant_tenancy from 002_tenancy.sql: do not call it before that file has run.
CREATE OR REPLACE PROCEDURE platform.provision_service(service text)
LANGUAGE plpgsql
AS $$
DECLARE
  role_name text := 'svc_' || service;
BEGIN
  IF service !~ '^[a-z][a-z0-9_]{1,30}$' OR service LIKE 'pg\_%' OR service IN ('platform', 'public') THEN
    RAISE EXCEPTION 'invalid service name: %', service;
  END IF;

  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = role_name) THEN
    EXECUTE format(
      'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
      role_name);
  END IF;

  EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I AUTHORIZATION timelex_migrator', service);
  EXECUTE format('REVOKE ALL ON SCHEMA %I FROM PUBLIC', service);
  EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', service, role_name);
  EXECUTE format('ALTER ROLE %I SET search_path = %I', role_name, service);
  EXECUTE format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), role_name);

  CALL platform.grant_tenancy(role_name);

  -- P1-06 (ADR-0014): every service appends to the platform outbox and may move only the delivery
  -- columns. The table is created by 003_outbox.sql, before this procedure is first called.
  IF to_regclass('platform.outbox') IS NOT NULL THEN
    EXECUTE format('GRANT SELECT, INSERT ON platform.outbox TO %I', role_name);
    EXECUTE format(
      'GRANT UPDATE (published_at, attempts, next_attempt_at, last_error) ON platform.outbox TO %I',
      role_name);
    EXECUTE format(
      'GRANT USAGE ON SEQUENCE %s TO %I',
      pg_get_serial_sequence('platform.outbox', 'seq'), role_name);
  END IF;
END
$$;
REVOKE ALL ON PROCEDURE platform.provision_service(text) FROM PUBLIC;
