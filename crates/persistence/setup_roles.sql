-- ============================================================================
-- yog-sothoth — Postgres roles setup
-- ============================================================================
-- Run ONCE per database, with admin privileges.
-- Replace the placeholder passwords with values stored in your secrets manager.
--
-- Scope:
--   yog_indexer  : RW on event tables, RO on user-facing tables
--   yog_api      : RO on event tables, will gain RW on user-facing tables in v0.3
--   yog_context  : RW on token enrichment tables, RO on pools (work queue)
--
-- Note: a separate migration role (yog_migrate) is intentionally NOT created
-- here. For v0.1, migrations are run by the admin role manually or via CI.
-- A dedicated migration role will be introduced when the deployment pipeline
-- justifies the separation (v0.3+).
-- ============================================================================

-- ---------------------------------------------------------------------------
-- Roles
-- ---------------------------------------------------------------------------
CREATE ROLE yog_indexer LOGIN PASSWORD 'CHANGE_ME_indexer_password';
CREATE ROLE yog_api     LOGIN PASSWORD 'CHANGE_ME_api_password';
CREATE ROLE yog_context LOGIN PASSWORD 'CHANGE_ME_context_password';

-- ---------------------------------------------------------------------------
-- Schema access
-- ---------------------------------------------------------------------------
GRANT USAGE ON SCHEMA public TO yog_indexer, yog_api, yog_context;

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
-- Default privileges for FUTURE tables created by the admin role.
-- This avoids forgetting GRANTs every time a migration adds a table.
-- ---------------------------------------------------------------------------
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

-- yog_context reads enrichment-adjacent tables (e.g. pools); a default
-- SELECT keeps it consistent with the other roles for future tables.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_context;

-- Indexer needs INSERT/UPDATE on event tables — grant explicitly per table
-- in each migration that creates one. Default privileges intentionally keep
-- INSERT/UPDATE narrow.

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;