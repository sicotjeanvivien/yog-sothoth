-- ============================================================================
-- 010 — meteora_damm_v2_swap_events_hourly (continuous aggregate)
-- ============================================================================
-- First continuous aggregate of the "rollups durables" plan (BACKLOG →
-- Continuous aggregates). Two roles:
--   1. durable history — survives the 30d retention drop on the raw swap
--      hypertable, holding hourly volume per pool indefinitely;
--   2. perf — feeds the volume_24h_usd computation of GET /api/pools without
--      scanning the raw swap rows.
--
-- USD is NOT stored here: a continuous aggregate cannot join token_prices.
-- We store RAW token amounts and value them at read time, per bucket, at the
-- price as-of that bucket — preserving the existing trade-time valuation
-- (value at the price when the trade happened, not the current price).
-- The valuation counts only the INPUT side of each swap, exactly like the
-- current read-time query (a_to_b → amount_a, b_to_a → amount_b), hence the
-- direction-filtered sums.
--
-- materialized_only = false → real-time aggregation: reads union the
-- materialized buckets with a live query over the not-yet-materialized recent
-- raw rows, so the current (partial) hour is always reflected.
--
-- WITH NO DATA + a refresh policy (never refresh_continuous_aggregate, which
-- cannot run in a transaction) so the statement stays valid inside the sqlx
-- migration transaction; the policy backfills and keeps it current.

CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                        AS bucket,
    pool_address,
    token_a_mint,
    token_b_mint,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address, token_a_mint, token_b_mint
WITH NO DATA;

-- Backfill + keep current. start_offset spans the full 30d retention window
-- (raw rows never live longer), end_offset leaves the current hour to
-- real-time aggregation, schedule_interval refreshes hourly.
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- Read-only: only the api role queries the rollup (pool_analytics).
GRANT SELECT ON meteora_damm_v2_swap_events_hourly TO yog_api;
