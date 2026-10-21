-- 031_swaps_valued_by_their_trusted_leg.sql — the tranches, their prices and the
-- facts summed from them were valued at the pool's mid price, which an empty pool
-- sets at will. Emptied, so the writer values them again from its floor, now by
-- each direction's trusted leg; the floor stays, the 5-minute aggregate covering it.

-- ⚠️ The periods too: written once at each freeze, they would refuse the hours frozen again.
TRUNCATE analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min,
         analytic_pool_hourly, analytic_protocol_hourly, analytic_token_periods,
         analytic_protocol_periods;

-- ⚠️ The aggregate's first day is skipped, as partial or being dropped by its
-- retention; with no aggregate at all, no hour stays.
WITH floor AS (
    SELECT CASE WHEN a.first IS NOT NULL THEN
               GREATEST((SELECT MIN(bucket) FROM analytic_hours),
                        date_trunc('day', a.first, 'UTC') + INTERVAL '1 day')
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
