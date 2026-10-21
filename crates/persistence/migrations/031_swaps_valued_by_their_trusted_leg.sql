-- 031_swaps_valued_by_their_trusted_leg.sql — the tranches, their prices, the facts
-- and the periods summed from them hold values at the pool's mid price, which an
-- empty pool sets at will. Emptied, so the writer values them again from its floor,
-- each direction by its gross trusted leg; the 5-minute aggregate, recreated, also
-- carries the B fees the buys pay on their input, which the gross needs.

DROP MATERIALIZED VIEW meteora_damm_v2_swap_events_5min;

-- ⚠️ The sums of the hourly aggregate (007), one for one: the facts read their
-- hours from here, and must equal what the hourly views value.
CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_5min
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('5 minutes', timestamp)                     AS bucket,
    pool_address,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE fee_token_is_a)                       AS fee_in_a,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE NOT fee_token_is_a)                   AS fee_in_b,
    SUM(protocol_fee) FILTER (WHERE fee_token_is_a)         AS protocol_fee_in_a,
    SUM(protocol_fee) FILTER (WHERE NOT fee_token_is_a)     AS protocol_fee_in_b,
    SUM(referral_fee) FILTER (WHERE fee_token_is_a)         AS referral_fee_in_a,
    SUM(referral_fee) FILTER (WHERE NOT fee_token_is_a)     AS referral_fee_in_b,
    SUM(amount_a)                                           AS traded_a,
    SUM(amount_b)                                           AS traded_b,
    -- The part of the B fees the buys pay on their input: the rest is the sales'.
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE trade_direction = 'b_to_a' AND NOT fee_token_is_a)
                                                            AS fee_in_b_on_buys,
    SUM(protocol_fee) FILTER (WHERE trade_direction = 'b_to_a' AND NOT fee_token_is_a)
                                                            AS protocol_fee_in_b_on_buys,
    SUM(referral_fee) FILTER (WHERE trade_direction = 'b_to_a' AND NOT fee_token_is_a)
                                                            AS referral_fee_in_b_on_buys
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

-- ⚠️ `end_offset` is the freeze margin: a row landing in a materialised tranche
-- stays unseen until the next refresh. `start_offset` stays under the raw 7 days (008).
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_5min',
    start_offset         => INTERVAL '6 days',
    end_offset           => INTERVAL '10 minutes',
    schedule_interval    => INTERVAL '5 minutes',
    -- ⚠️ Newest first, the watermark leads while older tranches are still empty.
    refresh_newest_first => false);

-- ⚠️ Past the raw 7 days: an hour caught up after a week-long outage reads its tranches.
SELECT add_retention_policy('meteora_damm_v2_swap_events_5min', INTERVAL '30 days');

GRANT SELECT ON meteora_damm_v2_swap_events_5min TO yog_analytic;

-- ⚠️ The periods too: written once at each freeze, they would refuse the hours frozen again.
TRUNCATE analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min,
         analytic_pool_hourly, analytic_protocol_hourly, analytic_token_periods,
         analytic_protocol_periods;

-- ⚠️ On the raw swaps, which the aggregate just recreated covers exactly: a MIN over its
-- real-time part would aggregate them all. Their first hour skipped, as partial; with none,
-- no hour stays.
WITH floor AS (
    SELECT CASE WHEN MIN(timestamp) IS NOT NULL THEN
               GREATEST((SELECT MIN(bucket) FROM analytic_hours),
                        date_trunc('hour', MIN(timestamp), 'UTC') + INTERVAL '1 hour')
           END AS bucket
      FROM meteora_damm_v2_swap_events
),
dropped AS (
    DELETE FROM analytic_hours
     WHERE bucket IS DISTINCT FROM (SELECT bucket FROM floor)
)
INSERT INTO analytic_hours (bucket)
SELECT bucket FROM floor WHERE bucket IS NOT NULL
    ON CONFLICT (bucket) DO NOTHING;
UPDATE analytic_hours SET frozen_at = NULL;
