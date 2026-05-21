-- ============================================================================
-- yog-sothoth — Postgres roles setup
-- ============================================================================
-- Run ONCE per database, with admin privileges.
-- Replace the placeholder passwords with values stored in your secrets manager.
--
-- Scope:
--   yog_migrate  : DDL — owns the schema evolution (CREATE / ALTER tables,
--                  GRANT statements emitted by migration files). Used by the
--                  yog-migrate binary; never by runtime services.
--   yog_indexer  : RW on event tables, RO on user-facing tables
--   yog_api      : RO on event tables, will gain RW on user-facing tables in v0.3
--   yog_context  : RW on token enrichment tables, RO on pools (work queue)
--
-- Least privilege at runtime: none of yog_indexer / yog_api / yog_context can
-- CREATE or ALTER tables. The day one of them is compromised, the schema
-- itself stays out of reach.
-- ============================================================================

-- ---------------------------------------------------------------------------
-- Roles
-- ---------------------------------------------------------------------------
CREATE ROLE yog_migrate LOGIN PASSWORD 'CHANGE_ME_migrate_password';
CREATE ROLE yog_indexer LOGIN PASSWORD 'CHANGE_ME_indexer_password';
CREATE ROLE yog_api     LOGIN PASSWORD 'CHANGE_ME_api_password';
CREATE ROLE yog_context LOGIN PASSWORD 'CHANGE_ME_context_password';

-- ---------------------------------------------------------------------------
-- Schema access
-- ---------------------------------------------------------------------------
GRANT USAGE  ON SCHEMA public TO yog_indexer, yog_api, yog_context;

-- yog_migrate needs USAGE + CREATE on the schema to apply DDL. It must also
-- be able to GRANT on the tables it creates so that migration files can emit
-- their own GRANT statements. The simplest way to get a clean ownership chain
-- is to make yog_migrate own the schema entirely.
ALTER SCHEMA public OWNER TO yog_migrate;
GRANT USAGE, CREATE ON SCHEMA public TO yog_migrate;

-- ---------------------------------------------------------------------------
-- Indexer grants
-- ---------------------------------------------------------------------------
-- Tables the indexer writes to: ingestion pipeline outputs.
-- `network_status` is written by the reporter task (slot + latency,
-- upserted every ~15s) — same RW profile as pool_current_state.
GRANT SELECT, INSERT, UPDATE
    ON pools, swap_events, liquidity_events,
       position_fee_claims, reward_claims, pool_current_state,
       network_status
    TO yog_indexer;

-- Tables the indexer only reads: allowlist filter applied at startup.
GRANT SELECT ON watched_pools TO yog_indexer;

-- Sequences associated with SERIAL/BIGSERIAL primary keys (if any).
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_indexer;

-- ---------------------------------------------------------------------------
-- API grants (v0.1 — read-only on event data, read-only on watched_pools)
-- ---------------------------------------------------------------------------
-- `network_status` is read by the GET /api/network/status handler.
-- Token enrichment tables (token_metadata, token_prices) will be read by
-- future token-detail / price endpoints — granting RO now keeps schema and
-- privileges aligned.
GRANT SELECT
    ON pools, swap_events, liquidity_events,
       position_fee_claims, reward_claims, watched_pools, pool_current_state,
       network_status, token_metadata, token_prices
    TO yog_api;

-- ---------------------------------------------------------------------------
-- Context grants
-- ---------------------------------------------------------------------------
-- Writes the token enrichment tables. SELECT is required on token_metadata
-- too: the metadata worker reads it (list_known_mints, and the NOT IN
-- sub-query of list_missing_mints).
GRANT SELECT, INSERT, UPDATE ON token_metadata TO yog_context;

-- token_prices is append-only — INSERT only suffices for the price worker,
-- but SELECT is granted alongside for symmetry and future read paths
-- (e.g. cross-checking a fresh fetch against the last known price).
GRANT SELECT, INSERT ON token_prices TO yog_context;

-- Reads `pools` to compute the metadata work queue (the list_missing_mints
-- query joins `pools` against `token_metadata`).
GRANT SELECT ON pools TO yog_context;

-- ---------------------------------------------------------------------------
-- Default privileges for FUTURE tables created by yog_migrate.
--
-- IMPORTANT: ALTER DEFAULT PRIVILEGES is scoped to the role that creates
-- the objects. Since yog_migrate now owns the schema and applies all
-- migrations, the defaults must be set FOR ROLE yog_migrate — otherwise a
-- table created by a migration would not inherit these grants.
-- ---------------------------------------------------------------------------
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

-- yog_context reads enrichment-adjacent tables (e.g. pools); a default
-- SELECT keeps it consistent with the other roles for future tables.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_context;

-- Indexer needs INSERT/UPDATE on event tables — those are granted explicitly
-- inside each migration that creates such a table. Default privileges
-- intentionally keep INSERT/UPDATE narrow.

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;