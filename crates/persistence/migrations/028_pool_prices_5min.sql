-- 028_pool_prices_5min.sql — the price of each pool's two tokens per 5-minute
-- tranche, written once by yog-analytic, and the swaps valued from it; the
-- hourly facts become their sums, and are rebuilt from the second day the raw
-- swaps still cover.

CREATE TABLE analytic_tranches (
    bucket     TIMESTAMPTZ PRIMARY KEY,
    written_at TIMESTAMPTZ NOT NULL
);

-- `market_*` is the market price, `price_*` the one published: their gap is
-- how far the pool and the market disagree.
CREATE TABLE analytic_pool_prices_5min (
    pool_address TEXT        NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    market_a_usd NUMERIC,
    price_a_usd  NUMERIC,
    market_b_usd NUMERIC,
    price_b_usd  NUMERIC,
    PRIMARY KEY (pool_address, bucket)
);

SELECT create_hypertable('analytic_pool_prices_5min', 'bucket',
                         chunk_time_interval => INTERVAL '1 day',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_pool_prices_5min_bucket ON analytic_pool_prices_5min (bucket DESC);

-- The USD columns are NULL exactly when `valued` is false.
CREATE TABLE analytic_pool_5min (
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

SELECT create_hypertable('analytic_pool_5min', 'bucket',
                         chunk_time_interval => INTERVAL '1 day',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_pool_5min_bucket ON analytic_pool_5min (bucket DESC);

-- ⚠️ As long as the raw swaps (014): an older tranche cannot be valued again.
SELECT add_retention_policy('analytic_pool_prices_5min', INTERVAL '7 days');
SELECT add_retention_policy('analytic_pool_5min', INTERVAL '7 days');

-- Written once: no UPDATE, so a second write fails in Postgres itself.
GRANT SELECT, INSERT
    ON analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min
    TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_swap_events TO yog_analytic;
GRANT SELECT ON analytic_hours TO yog_signals;

CREATE OR REPLACE VIEW meteora_damm_v2_pool_hourly_flow AS
SELECT
    pool_address,
    bucket,
    volume_in_a_usd AS volume_a_to_b_usd,
    volume_in_b_usd AS volume_b_to_a_usd,
    valued          AS valuation_complete
FROM analytic_pool_hourly;

-- ⚠️ The floor skips the oldest raw day, which the retention may drop during the
-- rebuild; only that hour stays, unfrozen. With no raw swap, nothing stays.
TRUNCATE analytic_pool_hourly, analytic_protocol_hourly;
DELETE FROM analytic_hours
 WHERE bucket IS DISTINCT FROM (
     SELECT CASE WHEN MIN(timestamp) IS NOT NULL THEN
                GREATEST(date_trunc('day', MIN(timestamp), 'UTC') + INTERVAL '1 day',
                         (SELECT MIN(bucket) FROM analytic_hours))
            END
       FROM meteora_damm_v2_swap_events);
-- A writer stopped longer than the raw retention has no hour at the floor yet.
INSERT INTO analytic_hours (bucket)
SELECT date_trunc('day', MIN(timestamp), 'UTC') + INTERVAL '1 day'
  FROM meteora_damm_v2_swap_events
HAVING MIN(timestamp) IS NOT NULL AND NOT EXISTS (SELECT 1 FROM analytic_hours);
UPDATE analytic_hours SET frozen_at = NULL;
