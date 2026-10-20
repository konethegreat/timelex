-- Written by Kone Tshivhinda & Claude
-- P1-02: tenancy enforced by the database (row-level security), not only by application code.
--
-- How a request works: the service opens a transaction and runs
--     SELECT set_config('timelex.tenant_id', '<tenant uuid>', true);   -- true = this transaction only
-- Every policy then compares a row's tenant_id with platform.current_tenant(). If the setting is
-- missing, current_tenant() is NULL, the comparison is never true, and the query sees nothing
-- (fail closed). A bad uuid raises an error, which also fails closed.

-- 1. The tenant the current transaction is acting for (NULL when none is set).
CREATE OR REPLACE FUNCTION platform.current_tenant()
RETURNS uuid
LANGUAGE sql
STABLE
PARALLEL SAFE
SET search_path = pg_catalog
AS $$
  SELECT NULLIF(current_setting('timelex.tenant_id', true), '')::uuid
$$;
REVOKE ALL ON FUNCTION platform.current_tenant() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION platform.current_tenant() TO timelex_migrator;

-- 2. Lets a service role call current_tenant() (policies are evaluated with the caller's rights).
CREATE OR REPLACE PROCEDURE platform.grant_tenancy(role_name name)
LANGUAGE plpgsql
AS $$
BEGIN
  EXECUTE format('GRANT USAGE ON SCHEMA platform TO %I', role_name);
  EXECUTE format('GRANT EXECUTE ON FUNCTION platform.current_tenant() TO %I', role_name);
END
$$;
REVOKE ALL ON PROCEDURE platform.grant_tenancy(name) FROM PUBLIC;

-- 3. The one policy every tenant-owned table gets. Migrations call this instead of writing their
--    own policy, so every table is protected identically.
--      ENABLE  turns row-level security on for ordinary roles.
--      FORCE   applies it to the table owner too, so a mistake in a migration cannot bypass it.
--    "(SELECT ...)" makes Postgres evaluate the function once per query, not once per row.
CREATE OR REPLACE PROCEDURE platform.apply_tenant_rls(tbl regclass, tenant_column name DEFAULT 'tenant_id')
LANGUAGE plpgsql
AS $$
BEGIN
  EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tbl);
  EXECUTE format('ALTER TABLE %s FORCE ROW LEVEL SECURITY', tbl);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %s', tbl);
  EXECUTE format(
    'CREATE POLICY tenant_isolation ON %s '
    'USING (%I = (SELECT platform.current_tenant())) '
    'WITH CHECK (%I = (SELECT platform.current_tenant()))',
    tbl, tenant_column, tenant_column);
END
$$;
GRANT USAGE ON SCHEMA platform TO timelex_migrator;
GRANT EXECUTE ON PROCEDURE platform.apply_tenant_rls(regclass, name) TO timelex_migrator;
REVOKE ALL ON PROCEDURE platform.apply_tenant_rls(regclass, name) FROM PUBLIC;

-- 4. A safety net for CI: lists every table in a service schema that does NOT have the standard
--    policy. The test suite asserts this view is empty, so a new table cannot ship unprotected.
--    (_sqlx_migrations is the migration tool's own bookkeeping table and holds no tenant data.)
CREATE OR REPLACE VIEW platform.tables_without_tenant_rls AS
SELECT n.nspname AS schema_name, c.relname AS table_name
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'platform', 'public')
  AND n.nspname NOT LIKE 'pg\_%'
  AND c.relname <> '_sqlx_migrations'
  AND NOT (
    c.relrowsecurity
    AND c.relforcerowsecurity
    AND EXISTS (SELECT 1 FROM pg_policy p WHERE p.polrelid = c.oid AND p.polname = 'tenant_isolation')
  );
REVOKE ALL ON platform.tables_without_tenant_rls FROM PUBLIC;
GRANT SELECT ON platform.tables_without_tenant_rls TO timelex_migrator;
