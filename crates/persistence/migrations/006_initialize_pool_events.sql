-- ============================================================================
-- 006 — meteora_damm_v2_initialize_pool_events
-- ============================================================================
-- Pool genesis (ring 2). Emitted once when a DAMM v2 pool is created. Carries
-- both mints, the initial AMM state (sqrt price + bounds, seeded liquidity),
-- the activation schedule, and the seeded token amounts.
--
-- "voie C": the fee configuration is captured undecoded as a raw borsh blob
-- (pool_fees_raw BYTEA). The fee_tier derived from it is a separate, later
-- piece of work that reads these stored bytes.
--
-- u128 -> NUMERIC(39,0); u8 -> SMALLINT; u64 -> BIGINT.

CREATE TABLE meteora_damm_v2_initialize_pool_events (
    id                  BIGSERIAL,
    pool_address        TEXT           NOT NULL,
    signature           TEXT           NOT NULL,

    token_a_mint        TEXT           NOT NULL,
    token_b_mint        TEXT           NOT NULL,
    creator             TEXT           NOT NULL,
    payer               TEXT           NOT NULL,
    alpha_vault         TEXT           NOT NULL,

    sqrt_min_price      NUMERIC(39, 0) NOT NULL,
    sqrt_max_price      NUMERIC(39, 0) NOT NULL,
    sqrt_price          NUMERIC(39, 0) NOT NULL,
    liquidity           NUMERIC(39, 0) NOT NULL,

    activation_type     SMALLINT       NOT NULL,
    activation_point    BIGINT         NOT NULL,
    collect_fee_mode    SMALLINT       NOT NULL,
    pool_type           SMALLINT       NOT NULL,

    token_a_flag        SMALLINT       NOT NULL,
    token_b_flag        SMALLINT       NOT NULL,
    token_a_amount      BIGINT         NOT NULL,
    token_b_amount      BIGINT         NOT NULL,
    total_amount_a      BIGINT         NOT NULL,
    total_amount_b      BIGINT         NOT NULL,

    pool_fees_raw       BYTEA          NOT NULL,

    timestamp           TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_initialize_pool_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_initialize_pool_events (pool_address, timestamp DESC);

CREATE UNIQUE INDEX ON meteora_damm_v2_initialize_pool_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_initialize_pool_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_initialize_pool_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_damm_v2_initialize_pool_events',   INTERVAL '30 days');

-- SELECT and sequence USAGE inherited from default privileges; only
-- INSERT/UPDATE granted explicitly.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_initialize_pool_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_initialize_pool_events TO yog_api;
