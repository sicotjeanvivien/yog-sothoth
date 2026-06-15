-- ============================================================================
-- 011 — meteora_damm_v2_liquidity_events_hourly (continuous aggregate)
-- ============================================================================
-- Second durable rollup (BACKLOG → Continuous aggregates). History-only: no
-- hot read path consumes it yet — its job is to survive the 30d retention
-- drop on the raw liquidity hypertable so the per-pool add/remove history is
-- not lost.
--
-- Like the swap CA values only the input side via direction-filtered sums,
-- liquidity events carry a direction in `liquidity_event_kind ∈ ('add',
-- 'remove')` and `liquidity_delta` is an unsigned magnitude (u128) — summing
-- it across both kinds would be meaningless, so every value is split by kind.
--
-- Raw token amounts / liquidity deltas only; any USD valuation stays at read
-- time (a CAGG cannot join token_prices), hence token_a_mint / token_b_mint
-- are carried for a future conversion. materialized_only = false for
-- consistency with the swap CA (reads reflect the current partial hour).
--
-- WITH NO DATA + refresh policy (never refresh_continuous_aggregate, which
-- cannot run in the sqlx migration transaction); the policy backfills.

CREATE MATERIALIZED VIEW meteora_damm_v2_liquidity_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                              AS bucket,
    pool_address,
    token_a_mint,
    token_b_mint,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_a_added,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_b_added,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_a_removed,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_b_removed,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'add')    AS liquidity_added,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'remove') AS liquidity_removed,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'add')    AS add_count,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'remove') AS remove_count
FROM meteora_damm_v2_liquidity_events
GROUP BY bucket, pool_address, token_a_mint, token_b_mint
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_liquidity_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_liquidity_events_hourly TO yog_api;
