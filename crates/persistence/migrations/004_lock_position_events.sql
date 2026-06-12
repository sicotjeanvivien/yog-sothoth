-- ============================================================================
-- 004 — meteora_damm_v2_lock_position_events
-- ============================================================================
-- Position lifecycle (ring 2). Emitted when an LP locks a position under a
-- vesting schedule: cliff_unlock_liquidity unlocks at cliff_point, then
-- liquidity_per_period every period_frequency for number_of_period periods.
--
-- The two liquidity fields are lossless u128 → NUMERIC(39, 0), matching the
-- liquidity_delta / next_sqrt_price convention. number_of_period is u16, so
-- INTEGER (SMALLINT/i16 cannot hold the full u16 range).

CREATE TABLE meteora_damm_v2_lock_position_events (
    id                       BIGSERIAL,
    pool_address             TEXT           NOT NULL,
    signature                TEXT           NOT NULL,

    position                 TEXT           NOT NULL,
    owner                    TEXT           NOT NULL,
    vesting                  TEXT           NOT NULL,

    cliff_point              BIGINT         NOT NULL,
    period_frequency         BIGINT         NOT NULL,
    cliff_unlock_liquidity   NUMERIC(39, 0) NOT NULL,
    liquidity_per_period     NUMERIC(39, 0) NOT NULL,
    number_of_period         INTEGER        NOT NULL,

    timestamp                TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_lock_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_lock_position_events (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_lock_position_events (position, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_lock_position_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_lock_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_lock_position_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_lock_position_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges; only
-- INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_lock_position_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_lock_position_events TO yog_api;
