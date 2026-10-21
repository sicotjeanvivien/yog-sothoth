-- 031_swaps_valued_by_their_trusted_leg.sql — the 5-minute aggregate recreated: it also
-- carries the B fees the buys pay on their input, which the gross of each direction needs.
-- 032 materialises it; 033 gives it back its policies and values the past again from it.

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

-- ⚠️ No policy before 032 has materialised it: a refresh running alongside would race it (033).
GRANT SELECT ON meteora_damm_v2_swap_events_5min TO yog_analytic;
