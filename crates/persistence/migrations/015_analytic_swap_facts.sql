-- 015_analytic_swap_facts.sql — the swaps valued once, per pool and hour, by
-- yog-analytic, the protocol aggregate drawn from them, and the protocol TVL.
-- An hour is rewritten at every cycle until it freezes, then never again;
-- `analytic_hours` records which. Only yog_analytic writes here, by upsert.

CREATE TABLE analytic_hours (
    bucket    TIMESTAMPTZ PRIMARY KEY,
    frozen_at TIMESTAMPTZ
);

-- The USD columns are NULL exactly when `valued` is false.
CREATE TABLE analytic_pool_hourly (
    pool_address      TEXT        NOT NULL,
    bucket            TIMESTAMPTZ NOT NULL,
    protocol          TEXT        NOT NULL,
    swap_count        BIGINT      NOT NULL,
    valued            BOOLEAN     NOT NULL,
    volume_in_a_usd   NUMERIC,
    volume_in_b_usd   NUMERIC,
    fees_usd          NUMERIC,
    lp_fees_usd       NUMERIC,
    protocol_fees_usd NUMERIC,
    referral_fees_usd NUMERIC,
    PRIMARY KEY (pool_address, bucket)
);

SELECT create_hypertable('analytic_pool_hourly', 'bucket',
                         chunk_time_interval => INTERVAL '7 days',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_pool_hourly_bucket ON analytic_pool_hourly (bucket DESC);

ALTER TABLE analytic_pool_hourly SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'bucket DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

SELECT add_compression_policy('analytic_pool_hourly', INTERVAL '7 days');

CREATE TABLE analytic_protocol_hourly (
    protocol            TEXT        NOT NULL,
    bucket              TIMESTAMPTZ NOT NULL,
    volume_usd          NUMERIC,
    fees_usd            NUMERIC,
    lp_fees_usd         NUMERIC,
    protocol_fees_usd   NUMERIC,
    referral_fees_usd   NUMERIC,
    swap_count          BIGINT      NOT NULL,
    active_pools        BIGINT      NOT NULL,
    active_pools_valued BIGINT      NOT NULL,
    PRIMARY KEY (protocol, bucket)
);

CREATE TABLE analytic_protocol_tvl (
    protocol     TEXT        NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    tvl_usd      NUMERIC,
    pools_priced BIGINT      NOT NULL,
    measured_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (protocol, bucket)
);

GRANT SELECT, INSERT, UPDATE
    ON analytic_hours, analytic_pool_hourly, analytic_protocol_hourly, analytic_protocol_tvl
    TO yog_analytic;
