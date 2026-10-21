-- 031_swaps_valued_by_their_trusted_leg.sql — the tranches, their prices and the
-- facts summed from them were valued at the pool's mid price, which an empty pool
-- sets at will. Emptied, so the writer values them again from its floor, now by
-- each direction's trusted leg; the floor stays, the 5-minute aggregate covering it.

TRUNCATE analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min,
         analytic_pool_hourly, analytic_protocol_hourly, analytic_token_periods;

-- ⚠️ The aggregate's first day is skipped, as partial or being dropped by its
-- retention; GREATEST ignores the NULL of an empty aggregate or of no hour at all.
WITH floor AS (
    SELECT GREATEST(
               (SELECT MIN(bucket) FROM analytic_hours),
               (SELECT date_trunc('day', MIN(bucket), 'UTC') + INTERVAL '1 day'
                  FROM meteora_damm_v2_swap_events_5min)) AS bucket
),
dropped AS (
    DELETE FROM analytic_hours
     WHERE bucket IS DISTINCT FROM (SELECT bucket FROM floor)
)
INSERT INTO analytic_hours (bucket)
SELECT bucket FROM floor WHERE bucket IS NOT NULL
    ON CONFLICT (bucket) DO NOTHING;
UPDATE analytic_hours SET frozen_at = NULL;
