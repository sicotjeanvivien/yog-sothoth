-- ============================================================================
-- 008 — meteora_damm_v2_update_pool_fees_events
-- ============================================================================
-- Pool admin (ring 2). Emitted when an operator updates a pool's fee
-- parameters. "voie C": the new fee parameters are stored as a raw,
-- undecoded borsh blob (params_raw BYTEA) — the trailing
-- UpdatePoolFeesParameters of the wire event, captured verbatim. Decoding is
-- deferred to dedicated fee work.

CREATE TABLE meteora_damm_v2_update_pool_fees_events (
    id            BIGSERIAL,
    pool_address  TEXT        NOT NULL,
    signature     TEXT        NOT NULL,

    operator      TEXT        NOT NULL,
    params_raw    BYTEA       NOT NULL,

    timestamp     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_pool_fees_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_update_pool_fees_events (pool_address, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_update_pool_fees_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_update_pool_fees_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_pool_fees_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_update_pool_fees_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges; only
-- INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_update_pool_fees_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_update_pool_fees_events TO yog_api;
