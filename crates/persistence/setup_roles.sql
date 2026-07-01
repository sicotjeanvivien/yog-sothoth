-- ============================================================================
-- yog-sothoth — Postgres roles setup
-- ============================================================================
-- Provisioning script — run ONCE per database, as superuser, before any
-- migration is applied. Defines roles and structural privileges only; the
-- per-table GRANTs live in the migrations that create the tables.
--
-- Replace the placeholder passwords with values stored in your secrets
-- manager. They appear in plain text here only as a template.
--
-- Sequence on a fresh database:
--   1. createdb yog_sothoth (as admin)
--   2. psql ... -f setup_roles.sql (this file)
--   3. yog-migrate runs as yog_migrate, applies 001_initial_schema.sql,
--      then any subsequent 00X migration.
--
-- Scope:
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
-- ============================================================================

-- ---------------------------------------------------------------------------
-- Roles
-- ---------------------------------------------------------------------------
CREATE ROLE yog_migrate LOGIN PASSWORD 'CHANGE_ME_migrate_password';
CREATE ROLE yog_indexer LOGIN PASSWORD 'CHANGE_ME_indexer_password';
CREATE ROLE yog_api     LOGIN PASSWORD 'CHANGE_ME_api_password';
CREATE ROLE yog_context LOGIN PASSWORD 'CHANGE_ME_context_password';
CREATE ROLE yog_signals LOGIN PASSWORD 'CHANGE_ME_signals_password';

-- ---------------------------------------------------------------------------
-- Schema access
-- ---------------------------------------------------------------------------
GRANT USAGE ON SCHEMA public TO yog_indexer, yog_api, yog_context, yog_signals;

-- yog_migrate owns the schema. This is the cleanest way to give it GRANT
-- authority over the tables it creates (migration files emit their own
-- GRANT statements as yog_migrate).
ALTER SCHEMA public OWNER TO yog_migrate;
GRANT USAGE, CREATE ON SCHEMA public TO yog_migrate;

-- ---------------------------------------------------------------------------
-- Default privileges for FUTURE tables created by yog_migrate
--
-- IMPORTANT: ALTER DEFAULT PRIVILEGES is scoped to the role that creates
-- the objects. Since yog_migrate now owns the schema and applies all
-- migrations, the defaults must be set FOR ROLE yog_migrate — otherwise
-- tables created by migrations would not inherit these defaults.
--
-- The defaults cover SELECT only. INSERT / UPDATE are granted explicitly
-- per table inside the relevant migration file, where the intent is
-- visible next to the table definition.
-- ---------------------------------------------------------------------------
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_context;

-- yog_signals evaluates detectors by reading future read-sources (caggs, state,
-- prices). Its RW on `signals` is granted explicitly in that table's migration;
-- SELECT on existing read-sources is granted per-table when a detector needs it.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_signals;

-- Sequences (behind BIGSERIAL columns) are used by yog_indexer at insert
-- time. Default USAGE + SELECT keeps future tables consistent.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;