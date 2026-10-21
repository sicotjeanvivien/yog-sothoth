-- 031_swaps_valued_by_their_trusted_leg.sql — the tranches, their prices, the facts
-- and the periods summed from them hold values at the pool's mid price, which an
-- empty pool sets at will. Emptied, so the writer values them again from its floor,
-- each direction by its gross trusted leg; the floor stays where the raw swaps cover it.

-- ⚠️ The periods too: written once at each freeze, they would refuse the hours frozen again.
TRUNCATE analytic_tranches, analytic_pool_prices_5min, analytic_pool_5min,
         analytic_pool_hourly, analytic_protocol_hourly, analytic_token_periods,
         analytic_protocol_periods;

-- ⚠️ On the raw swaps, which split the B fees: their first day skipped, as being dropped
-- by their retention; with none, no hour stays.
WITH floor AS (
    SELECT CASE WHEN MIN(timestamp) IS NOT NULL THEN
               GREATEST((SELECT MIN(bucket) FROM analytic_hours),
                        date_trunc('day', MIN(timestamp), 'UTC') + INTERVAL '1 day')
           END AS bucket
      FROM meteora_damm_v2_swap_events
),
dropped AS (
    DELETE FROM analytic_hours
     WHERE bucket IS DISTINCT FROM (SELECT bucket FROM floor)
)
INSERT INTO analytic_hours (bucket)
SELECT bucket FROM floor WHERE bucket IS NOT NULL
    ON CONFLICT (bucket) DO NOTHING;
UPDATE analytic_hours SET frozen_at = NULL;
