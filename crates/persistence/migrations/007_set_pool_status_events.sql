-- ============================================================================
-- 007 — meteora_damm_v2_set_pool_status_events
-- ============================================================================
-- Pool admin (ring 2). Emitted when a pool's status flag is changed. `status`
-- is the raw on-chain byte, stored uninterpreted (u8 -> SMALLINT).

CREATE TABLE meteora_damm_v2_set_pool_status_events (
    id            BIGSERIAL,
    pool_address  TEXT        NOT NULL,
    signature     TEXT        NOT NULL,

    status        SMALLINT    NOT NULL,

    timestamp     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_set_pool_status_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_set_pool_status_events (pool_address, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_set_pool_status_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_set_pool_status_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_set_pool_status_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_set_pool_status_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges; only
-- INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_set_pool_status_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_set_pool_status_events TO yog_api;
