-- ============================================================================
-- 005 — meteora_damm_v2_permanent_lock_position_events
-- ============================================================================
-- Position lifecycle (ring 2). Emitted when an LP permanently locks part of
-- a position's liquidity (no vesting, never unlocks). lock_liquidity_amount
-- is the amount locked by this action; total_permanent_locked_liquidity is
-- the position's running total afterwards. Both lossless u128 -> NUMERIC.
--
-- No owner column — the on-chain event only carries pool and position.

CREATE TABLE meteora_damm_v2_permanent_lock_position_events (
    id                                  BIGSERIAL,
    pool_address                        TEXT           NOT NULL,
    signature                           TEXT           NOT NULL,

    position                            TEXT           NOT NULL,
    lock_liquidity_amount               NUMERIC(39, 0) NOT NULL,
    total_permanent_locked_liquidity    NUMERIC(39, 0) NOT NULL,

    timestamp                           TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_permanent_lock_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_permanent_lock_position_events (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_permanent_lock_position_events (position, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_permanent_lock_position_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_permanent_lock_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_permanent_lock_position_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_permanent_lock_position_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges; only
-- INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_permanent_lock_position_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_permanent_lock_position_events TO yog_api;
