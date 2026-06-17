-- ============================================================================
-- 017 — meteora_damm_v2_swap_events_hourly : add realized trading-fee sums
-- ============================================================================
-- Extends the swap volume CA (defined in 010, last rebuilt in 014 when the mint
-- columns moved to `pools`) with per-swap realized fee aggregation, so fee
-- revenue — and the LP-vs-protocol split — is queryable per pool/hour without
-- scanning the raw swap hypertable. This feeds the "realized fee analytics"
-- read paths (effective rate, protocol revenue) added in a later migration.
--
-- Forward-only: a continuous aggregate's column set can't be ALTERed, so we
-- DROP + recreate. This discards the materialized history of the previous CA —
-- acceptable in dev; the new policy backfills from the 30d still present in the
-- raw hypertable. The DROP is plain (no CASCADE): nothing depends on the CA in
-- SQL (the cross-protocol `swap_events` VIEW reads the raw table, not this CA),
-- exactly as relied on by migration 014's own DROP.
--
-- The recreated definition is the 014 shape (no mint columns; USD/mints joined
-- at read time via `pools`) PLUS four fee columns. Keeping volume + count makes
-- this a strict superset, so the existing pool_analytics read path is unaffected.
--
-- A swap charges its fee in exactly ONE token (A or B), per the pool's
-- collect_fee_mode and the trade direction — captured by fee_token_is_a. We
-- therefore sum fees split on that flag into fee_in_a / fee_in_b. protocol_fee
-- is summed separately so the LP share is (fee_in_x - protocol_fee_in_x) and the
-- effective rate is fee_in_x / volume_in_x within the same bucket and token.

DROP MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly;

CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                        AS bucket,
    pool_address,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE fee_token_is_a)                       AS fee_in_a,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE NOT fee_token_is_a)                   AS fee_in_b,
    SUM(protocol_fee) FILTER (WHERE fee_token_is_a)         AS protocol_fee_in_a,
    SUM(protocol_fee) FILTER (WHERE NOT fee_token_is_a)     AS protocol_fee_in_b
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

-- Backfill + keep current. Mirrors the policy from 010/014: start_offset spans
-- the full 30d raw retention, end_offset leaves the current hour to real-time
-- aggregation, hourly schedule.
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- Read-only: only the api role queries the rollup (pool_analytics).
GRANT SELECT ON meteora_damm_v2_swap_events_hourly TO yog_api;
