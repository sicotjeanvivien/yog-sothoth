-- 002_web_readonly_user.sql
--
-- Provision a dedicated read-only Postgres role for the Next.js web
-- application. The web process only consumes data already written by
-- the indexer, so no INSERT/UPDATE/DELETE rights are granted.
--
-- The role itself is created without a password here; the password is
-- expected to be set at provisioning time via an out-of-band command:
--
--   ALTER ROLE yog_web WITH LOGIN PASSWORD '<provisioned-secret>';
--
-- Keeping the secret out of the migration file lets the same migration
-- be replayed safely across environments (dev, staging, prod) without
-- leaking credentials into version control.
--
-- This migration is idempotent: it can be applied repeatedly without
-- error. New tables added later automatically inherit the SELECT grant
-- through `ALTER DEFAULT PRIVILEGES`.

-- Create the role only if it does not already exist. NOLOGIN is set
-- intentionally; the operator turns LOGIN on after assigning a secret.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = 'yog_web'
    ) THEN
        CREATE ROLE yog_web WITH NOLOGIN;
    END IF;
END
$$;

-- Grant CONNECT on the *current* database. `GRANT CONNECT ON DATABASE`
-- requires a literal database identifier, not a function call, so we
-- build the statement dynamically via `format(... %I ...)` which
-- properly quotes the identifier returned by `current_database()`.
-- Doing this in a DO block keeps the migration agnostic of the actual
-- database name (dev, staging, prod can all reuse the same file).
DO $$
BEGIN
    EXECUTE format(
        'GRANT CONNECT ON DATABASE %I TO yog_web',
        current_database()
    );
END
$$;

-- Allow the role to use the public schema.
GRANT USAGE ON SCHEMA public TO yog_web;

-- Grant SELECT on every existing table and sequence in the public
-- schema. New objects created later are covered by the default
-- privileges below.
GRANT SELECT ON ALL TABLES IN SCHEMA public TO yog_web;
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_web;

-- Default privileges — applies to objects created in the future by the
-- migration runner role. Adjust the FOR ROLE clause to match the role
-- that actually owns the tables (typically the indexer's role).
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_web;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON SEQUENCES TO yog_web;