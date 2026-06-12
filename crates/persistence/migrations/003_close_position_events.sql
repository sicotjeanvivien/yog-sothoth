-- ============================================================================
-- 003 — meteora_damm_v2_close_position_events
-- ============================================================================
-- Position lifecycle (ring 2). Emitted when an LP closes a position and the
-- position account is torn down on-chain. Same shape as the create-position
-- table (002); paired with it, the two delimit a position's lifespan.
--
-- Per-protocol table, consistent with the voie-3 strategy.

CREATE TABLE meteora_damm_v2_close_position_events (
    id                 BIGSERIAL,
    pool_address       TEXT        NOT NULL,
    signature          TEXT        NOT NULL,

    owner              TEXT        NOT NULL,
    position           TEXT        NOT NULL,
    position_nft_mint  TEXT        NOT NULL,

    timestamp          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_close_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_close_position_events (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_close_position_events (position, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_close_position_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_close_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_close_position_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_close_position_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges (setup_roles.sql);
-- only INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_close_position_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_close_position_events TO yog_api;
