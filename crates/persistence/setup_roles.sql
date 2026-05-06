-- ============================================================================
-- yog-sothoth — Postgres roles setup
-- ============================================================================
-- Run ONCE per database, with admin privileges.
-- Replace the placeholder passwords with values stored in your secrets manager.
--
-- Scope:
--   yog_indexer  : RW on event tables, RO on user-facing tables
--   yog_api      : RO on event tables, will gain RW on user-facing tables in v0.3
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

-- ---------------------------------------------------------------------------
-- Schema access
-- ---------------------------------------------------------------------------
GRANT USAGE ON SCHEMA public TO yog_indexer, yog_api;

-- ---------------------------------------------------------------------------
-- Indexer grants
-- ---------------------------------------------------------------------------
-- Tables the indexer writes to: ingestion pipeline outputs.
GRANT SELECT, INSERT, UPDATE
    ON pools, swap_events, liquidity_events,
       position_fee_claims, reward_claims
    TO yog_indexer;

-- Tables the indexer only reads: allowlist filter applied at startup.
GRANT SELECT ON watched_pools TO yog_indexer;

-- Sequences associated with SERIAL/BIGSERIAL primary keys (if any).
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_indexer;

-- ---------------------------------------------------------------------------
-- API grants (v0.1 — read-only on event data, read-only on watched_pools)
-- ---------------------------------------------------------------------------
GRANT SELECT
    ON pools, swap_events, liquidity_events,
       position_fee_claims, reward_claims, watched_pools
    TO yog_api;

-- ---------------------------------------------------------------------------
-- Default privileges for FUTURE tables created by the admin role.
-- This avoids forgetting GRANTs every time a migration adds a table.
-- ---------------------------------------------------------------------------
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

-- Indexer needs INSERT/UPDATE on event tables — grant explicitly per table
-- in each migration that creates one. Default privileges intentionally keep
-- INSERT/UPDATE narrow.

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;