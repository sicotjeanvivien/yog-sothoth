-- 021_swap_events_5min.sql — the swaps per pool and 5 minutes, under the
-- analytic facts: a cycle reads its hours from these tranches, and the raw
-- swaps only past the last one materialised. The hourly aggregate is untouched.

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
    SUM(amount_b)                                           AS traded_b
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

-- ⚠️ `end_offset` is the freeze margin: a row landing in a materialised tranche
-- stays unseen until the next refresh. `start_offset` stays under the raw 7 days (008).
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_5min',
    start_offset      => INTERVAL '6 days',
    end_offset        => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '5 minutes');

-- ⚠️ Past the raw 7 days: an hour caught up after a week-long outage reads its tranches.
SELECT add_retention_policy('meteora_damm_v2_swap_events_5min', INTERVAL '30 days');

-- ⚠️ The price tables too: the facts read them directly, not through a view.
GRANT SELECT ON meteora_damm_v2_swap_events_5min, token_metadata, token_prices TO yog_analytic;
