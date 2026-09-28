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
--   Every runtime role below reads every table: the default privileges at
--   the end of this file grant SELECT to all four. What differs is what each
--   one writes, and the per-table grants in the migrations are the list.
--
--   yog_indexer  : writes the event tables, pools, pool_current_state and
--                  network_status.
--   yog_api      : writes nothing.
--   yog_context  : writes the token enrichment tables, the pool-properties
--                  satellites and the pool-property columns of pools.
--   yog_signals  : appends to signals.
--   yog_archive  : RO on everything, writes nothing. Used by yog-archive to
--                  run `pg_dump`, which must read every table — TimescaleDB's
--                  chunks and catalog included — so it is a member of the
--                  predefined `pg_read_all_data` rather than of per-table
--                  grants that every new table would have to remember.
--
-- Least privilege at runtime: none of yog_indexer / yog_api / yog_context /
-- yog_signals / yog_archive can CREATE or ALTER tables. The day one of them is compromised,
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
        'yog_signals', 'CHANGE_ME_signals_password',
        'yog_archive', 'CHANGE_ME_archive_password'
    );
BEGIN
    FOREACH role_name IN ARRAY ARRAY[
        'yog_migrate', 'yog_indexer', 'yog_api', 'yog_context', 'yog_signals',
        'yog_archive'
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


-- yog_archive reads everything and writes nothing, in every database of the
-- cluster: `pg_read_all_data` (Postgres >= 14) is SELECT on all tables, views
-- and sequences, and USAGE on all schemas, present and future. Membership is
-- cluster-scoped like the role itself; granting it again is a no-op.
GRANT pg_read_all_data TO yog_archive;


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


-- ---------------------------------------------------------------------------
-- What PUBLIC holds by default, taken back — DATABASE scope
--
-- Two rights every role receives through PUBLIC let a runtime role write,
-- where the matrix says it writes nothing (yog_api, yog_archive) or only its
-- own tables:
--
--   * TimescaleDB's job API. Every function of the extension is executable by
--     PUBLIC, so `add_job` succeeded under yog_api and yog_archive — every
--     second, on `pg_sleep` (measured 25 Sept 2026). A job runs with its
--     owner's rights, so it cannot reach a table the role could not; what it
--     can do is persist in the scheduler's catalog past a password change and
--     hold the workers that compress, drop and materialise. Nothing here uses
--     the four routines: the migrations go through the `add_*_policy`
--     functions, which do not call them.
--     ⚠️ yog_migrate loses them too. A future migration that calls one —
--     `alter_job` is the usual way to reschedule a policy — needs its own
--     `GRANT EXECUTE … TO yog_migrate` first, and CI will not tell you:
--     `sqlx::test` applies migrations as the superuser.
--   * TEMP on the database, a Postgres default: every role could create
--     temporary tables.
--
-- The extension is created first so the REVOKE does not depend on the image
-- having installed it: the migrations create it too, but they run after this
-- file. `ROUTINE`, not `FUNCTION`: `run_job` is a procedure, and one wrong kind
-- fails the whole statement. No signatures, so the names survive a change of
-- arguments.
--
-- ⚠️ An `ALTER EXTENSION timescaledb UPDATE` can undo this. Measured going
-- from 2.27.1 to 2.30.1: `alter_job` gains an argument, is recreated, and comes
-- back executable by PUBLIC; the other three keep the REVOKE. Re-run this file
-- after every extension update — it is idempotent.
-- ---------------------------------------------------------------------------
CREATE EXTENSION IF NOT EXISTS timescaledb;

REVOKE EXECUTE ON ROUTINE add_job, alter_job, delete_job, run_job FROM PUBLIC;

DO $$
BEGIN
    EXECUTE format('REVOKE TEMPORARY ON DATABASE %I FROM PUBLIC', current_database());
END $$;
