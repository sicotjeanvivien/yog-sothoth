-- ============================================================================
-- 013 — meteora_damm_v2_claim_reward_events_hourly (continuous aggregate)
-- ============================================================================
-- Fourth and last durable rollup (BACKLOG → Continuous aggregates),
-- history-only: an hourly CA over the raw reward-claim hypertable so per-pool
-- realized reward history survives the 30d retention drop. No read path
-- consumes it yet.
--
-- A pool can emit rewards in several tokens, so the rollup is grouped by
-- mint_reward in addition to the bucket — summing total_reward across distinct
-- reward tokens would be meaningless. Raw amounts only; USD valuation stays at
-- read time. materialized_only = false for consistency with the other CAs.
--
-- WITH NO DATA + refresh policy (never refresh_continuous_aggregate inside the
-- sqlx migration transaction); the policy backfills.

CREATE MATERIALIZED VIEW meteora_damm_v2_claim_reward_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    pool_address,
    mint_reward,
    SUM(total_reward)                AS total_reward,
    COUNT(*)                         AS claim_count
FROM meteora_damm_v2_claim_reward_events
GROUP BY bucket, pool_address, mint_reward
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_reward_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_claim_reward_events_hourly TO yog_api;
