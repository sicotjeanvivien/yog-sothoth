-- ============================================================================
-- 012 — meteora_damm_v2_claim_position_fee_events_hourly (continuous aggregate)
-- ============================================================================
-- Third durable rollup (BACKLOG → Continuous aggregates), history-only: an
-- hourly CA over the raw position-fee-claim hypertable so per-pool realized
-- fee history survives the 30d retention drop. No read path consumes it yet.
--
-- No direction to split (a claim is a claim) and the source table carries no
-- token mints — only fee_a_claimed / fee_b_claimed in raw units. USD valuation
-- (and the mints, via pools) stays at read time if ever needed; a CAGG cannot
-- join. materialized_only = false for consistency with the other CAs.
--
-- WITH NO DATA + refresh policy (never refresh_continuous_aggregate inside the
-- sqlx migration transaction); the policy backfills.

CREATE MATERIALIZED VIEW meteora_damm_v2_claim_position_fee_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    pool_address,
    SUM(fee_a_claimed)               AS fee_a_claimed,
    SUM(fee_b_claimed)               AS fee_b_claimed,
    COUNT(*)                         AS claim_count
FROM meteora_damm_v2_claim_position_fee_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_position_fee_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_claim_position_fee_events_hourly TO yog_api;
