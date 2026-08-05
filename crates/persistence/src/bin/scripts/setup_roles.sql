-- ============================================================================
-- yog-sothoth — Postgres roles and structural privileges
-- ============================================================================
-- Provisioning script, run as an admin/superuser BEFORE any migration:
--
--     cargo run -p yog-persistence --bin yog-migrate -- setup-roles
--
-- (or by hand: psql "$DATABASE_URL_ADMIN" -f crates/persistence/src/bin/scripts/setup_roles.sql)
--
-- Replace the placeholder passwords with values from your secrets manager.
-- They appear in plain text here only as a template.
--
-- ## Two scopes in one file, and why re-running is safe
--
-- Roles are **cluster-wide**; everything below them is **per-database**. That
-- asymmetry used to make this file un-rerunnable: bootstrapping a second
-- database in the same cluster re-ran `CREATE ROLE` on roles that already
-- existed and aborted on `role "yog_migrate" already exists`, before reaching
-- the per-database half that was the whole point of running it. The fix is the
-- guarded block below — the file is now idempotent, so:
--
--   * re-running it against the same database is a no-op;
--   * running it against a NEW database of the same cluster creates no role and
--     applies the per-database privileges, which is exactly what is needed.
--
-- ⚠️ It deliberately does **not** update an existing role's password. A rerun
-- must never silently reset a production credential to `CHANGE_ME_…`. Change a
-- password with an explicit `ALTER ROLE … PASSWORD …`, never by re-running this.
--
-- ## Scope
--
--   yog_migrate  : DDL — owns the schema, applies migrations.
--                  Used by the yog-migrate binary; never by runtime services.
--   yog_indexer  : RW on event tables, RO on watched_pools.
--   yog_api      : RO across the board.
--   yog_context  : RW on token enrichment tables, RO on pools.
--   yog_signals  : RW (append-only) on signals, RO on the read sources it
--                  evaluates (caggs, pool_current_state, token_prices).
--
-- Least privilege at runtime: none of yog_indexer / yog_api / yog_context /
-- yog_signals can CREATE or ALTER tables. The day one of them is compromised,
-- the schema itself stays out of reach.
--
-- Sequence on a fresh database:
--   1. createdb yog_sothoth (as admin) — not covered here, this file assumes
--      the database exists and connects to it.
--   2. this file
--   3. the migrations, as yog_migrate
--   4. setup_watched_pools.sql, to give a pool-centric indexer something to
--      subscribe to
--
-- Steps 2-4 in one go: `yog-migrate -- bootstrap`.
-- ============================================================================


-- ---------------------------------------------------------------------------
-- Roles — CLUSTER scope. Created once per cluster, not once per database.
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    role_name TEXT;
    -- Password only ever applies to a role this block actually creates.
    passwords CONSTANT JSONB := jsonb_build_object(
        'yog_migrate', 'CHANGE_ME_migrate_password',
        'yog_indexer', 'CHANGE_ME_indexer_password',
        'yog_api',     'CHANGE_ME_api_password',
        'yog_context', 'CHANGE_ME_context_password',
        'yog_signals', 'CHANGE_ME_signals_password'
    );
BEGIN
    FOREACH role_name IN ARRAY ARRAY[
        'yog_migrate', 'yog_indexer', 'yog_api', 'yog_context', 'yog_signals'
    ] LOOP
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
            RAISE NOTICE 'role % already exists — left untouched (password included)', role_name;
        ELSE
            EXECUTE format(
                'CREATE ROLE %I LOGIN PASSWORD %L',
                role_name, passwords ->> role_name
            );
            RAISE NOTICE 'role % created', role_name;
        END IF;
    END LOOP;
END $$;


-- ---------------------------------------------------------------------------
-- Schema access — DATABASE scope. Re-run for every database of the cluster.
-- ---------------------------------------------------------------------------
GRANT USAGE ON SCHEMA public TO yog_indexer, yog_api, yog_context, yog_signals;

-- yog_migrate owns the schema. This is the cleanest way to give it GRANT
-- authority over the tables it creates (the baseline migration emits its own
-- GRANT statements as yog_migrate).
ALTER SCHEMA public OWNER TO yog_migrate;
GRANT USAGE, CREATE ON SCHEMA public TO yog_migrate;


-- ---------------------------------------------------------------------------
-- Default privileges for FUTURE tables created by yog_migrate — DATABASE scope
--
-- IMPORTANT: ALTER DEFAULT PRIVILEGES is scoped to the role that creates
-- the objects. Since yog_migrate owns the schema and applies all
-- migrations, the defaults must be set FOR ROLE yog_migrate — otherwise
-- tables created by migrations would not inherit these defaults.
--
-- The defaults cover SELECT only. INSERT / UPDATE are granted explicitly
-- per table inside the migration, where the intent is visible next to the
-- table definition.
--
-- ⚠️ These defaults are also a blind spot. They make a *lost* explicit grant
-- indistinguishable from a held one — an ACL records "yog_api has SELECT", not
-- where it came from — which is how migration 014 dropped a grant that nobody
-- noticed for two months (see `migrations/001_baseline.sql` §14). The guard is
-- `tests/privileges.rs`, whose databases have no default privileges.
-- ---------------------------------------------------------------------------
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_context;

-- yog_signals evaluates detectors by reading future read-sources (caggs, state,
-- prices). Its RW on `signals` is granted explicitly in the baseline; SELECT on
-- existing read-sources is granted per-table when a detector needs it.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_signals;

-- Sequences (behind BIGSERIAL columns) are used by yog_indexer at insert
-- time. Default USAGE + SELECT keeps future tables consistent.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;
