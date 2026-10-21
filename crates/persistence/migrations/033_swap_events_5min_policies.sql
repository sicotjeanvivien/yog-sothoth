-- 033_swap_events_5min_policies.sql — once 032 has materialised the aggregate 031 recreated:
-- its policies, 021's, and the tranches, prices, facts and periods valued at the pool's mid
-- price, which an empty pool sets at will, emptied for the writer to value them again.

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

-- ⚠️ The periods too: written once at each freeze, they would refuse the hours frozen again.
TRUNCATE analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min,
         analytic_pool_hourly, analytic_protocol_hourly, analytic_token_periods,
         analytic_protocol_periods;

-- ⚠️ On the aggregate, materialised: a raw chunk dropped since 032 takes nothing from it.
-- Its first hour skipped, as partial; with no aggregate, no hour stays.
WITH floor AS (
    SELECT CASE WHEN a.first IS NOT NULL THEN
               GREATEST((SELECT MIN(bucket) FROM analytic_hours),
                        date_trunc('hour', a.first, 'UTC') + INTERVAL '1 hour')
           END AS bucket
      FROM (SELECT MIN(bucket) AS first FROM meteora_damm_v2_swap_events_5min) a
),
dropped AS (
    DELETE FROM analytic_hours
     WHERE bucket IS DISTINCT FROM (SELECT bucket FROM floor)
)
INSERT INTO analytic_hours (bucket)
SELECT bucket FROM floor WHERE bucket IS NOT NULL
    ON CONFLICT (bucket) DO NOTHING;
UPDATE analytic_hours SET frozen_at = NULL;
