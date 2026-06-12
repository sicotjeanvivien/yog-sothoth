-- ============================================================================
-- 002 — meteora_damm_v2_create_position_events
-- ============================================================================
-- Position lifecycle (ring 2). Emitted when an LP opens a new, empty
-- position on a pool. The position is NFT-backed (`position_nft_mint`);
-- `position` is the PDA holding its state. The event carries no token
-- amounts and no reserves — liquidity arrives later through a separate
-- liquidity event.
--
-- Per-protocol table, consistent with the voie-3 strategy and the table
-- shape established for the ring-1 event tables in 001.

CREATE TABLE meteora_damm_v2_create_position_events (
    id                 BIGSERIAL,
    pool_address       TEXT        NOT NULL,
    signature          TEXT        NOT NULL,

    owner              TEXT        NOT NULL,
    position           TEXT        NOT NULL,
    position_nft_mint  TEXT        NOT NULL,

    timestamp          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- Time-partition, matching the 7-day chunking of the ring-1 tables.
SELECT create_hypertable('meteora_damm_v2_create_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

-- Common access paths: by pool, and by position (LP dashboards).
CREATE INDEX ON meteora_damm_v2_create_position_events (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_create_position_events (position, timestamp DESC);

-- Idempotency: replay-safe inserts via ON CONFLICT DO NOTHING. Mirrors the
-- ring-1 convention. Hypertable unique indexes must include the partitioning
-- column, hence (signature, timestamp).
CREATE UNIQUE INDEX ON meteora_damm_v2_create_position_events (signature, timestamp);

-- Compression after 7 days, retention after 30 — same policy as ring-1.
ALTER TABLE meteora_damm_v2_create_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_create_position_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_create_position_events',   INTERVAL '30 days');

-- ----------------------------------------------------------------------------
-- Grants
-- ----------------------------------------------------------------------------
-- SELECT (yog_indexer/yog_api/yog_context) and sequence USAGE for yog_indexer
-- are inherited automatically from the ALTER DEFAULT PRIVILEGES FOR ROLE
-- yog_migrate set in setup_roles.sql. Only INSERT/UPDATE must be granted
-- explicitly, next to the table that needs them.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_create_position_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_create_position_events TO yog_api;
