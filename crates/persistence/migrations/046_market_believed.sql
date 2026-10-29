-- 046_market_believed.sql — one rule for a market price our pools contradict: yog_market_believed
-- gives none where it is more than yog_market_over_pool_max() times the token's ceiling (045).
-- `token_stock_price` holds a stock to it; yog-analytic holds a flow to it when it writes the flow,
-- once — yog_price_asof stays the raw quote, so what history shows never moves with a ceiling.

CREATE FUNCTION yog_market_believed(mint TEXT, price_usd NUMERIC)
RETURNS NUMERIC
LANGUAGE sql STABLE PARALLEL SAFE
RETURN CASE WHEN EXISTS (SELECT 1 FROM analytic_token_pool_ceilings c
                          WHERE c.mint = yog_market_believed.mint
                            AND yog_market_believed.price_usd > yog_market_over_pool_max() * c.ceiling_usd)
            THEN NULL ELSE yog_market_believed.price_usd END;

-- ⚠️ Called as its caller: every role reading `pool_current_tvl` reads the ceilings too.
GRANT SELECT ON analytic_token_pool_ceilings TO yog_api, yog_signals;

CREATE OR REPLACE VIEW token_stock_price AS
SELECT tp.mint, tp.price_usd, tp.fetched_at
  FROM token_last_price tp
 WHERE tp.fetched_at >= now() - yog_price_max_age_latest()
   AND yog_market_believed(tp.mint, tp.price_usd) IS NOT NULL;

-- A move in a pool that did not swap: its anchor held to the rule its tranche's swaps are.
CREATE OR REPLACE FUNCTION meteora_damm_v2_tranche_pool_prices(
    tranche        TIMESTAMPTZ,
    width          INTERVAL,
    pool_addresses TEXT[]
)
RETURNS TABLE (
    pool_address TEXT,
    price_a_usd  NUMERIC,
    price_b_usd  NUMERIC,
    market_a_usd NUMERIC,
    market_b_usd NUMERIC
)
LANGUAGE sql STABLE
BEGIN ATOMIC
WITH valued_pools AS (
    SELECT v.pool_address, p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b
      FROM unnest(pool_addresses) AS v (pool_address)
      LEFT JOIN pools p ON p.pool_address = v.pool_address
),
-- The anchor: the token in more pools of the tranche, these included; none on a tie.
pools_per_mint AS (
    SELECT m.mint, COUNT(*) AS pools
      FROM (SELECT s.pool_address, p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b
              FROM meteora_damm_v2_swap_events_5min s
              JOIN pools p ON p.pool_address = s.pool_address
             WHERE s.bucket = tranche
            UNION
            SELECT pool_address, mint_a, mint_b FROM valued_pools) t
     CROSS JOIN LATERAL (VALUES (t.mint_a), (t.mint_b)) m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY m.mint
),
swapped AS (
    SELECT q.pool_address, q.price_a_usd, q.price_b_usd, q.market_a_usd, q.market_b_usd
      FROM valued_pools mp
      JOIN analytic_pool_prices_5min q ON q.pool_address = mp.pool_address AND q.bucket = tranche
),
-- Without a swap: the anchor at market, the other at the lower of its market and the pool's
-- last bounded price. ⚠️ Never the pool's mid price; without a last price, no value.
unswapped AS (
    SELECT mp.pool_address,
           CASE WHEN n.anchor = 'a' THEN COALESCE(ma.price_usd, last_a.price)
                -- ⚠️ LEAST skips NULL: without a bound, the market alone would price the side.
                WHEN last_a.price IS NOT NULL THEN LEAST(ma.price_usd, last_a.price) END
               AS price_a_usd,
           CASE WHEN n.anchor = 'b' THEN COALESCE(mb.price_usd, last_b.price)
                WHEN last_b.price IS NOT NULL THEN LEAST(mb.price_usd, last_b.price) END
               AS price_b_usd,
           ma.price_usd AS market_a_usd,
           mb.price_usd AS market_b_usd
      FROM valued_pools mp
      LEFT JOIN LATERAL (
          SELECT q.price_a_usd AS price
            FROM analytic_pool_prices_5min q
           WHERE q.pool_address = mp.pool_address AND q.bucket < tranche AND q.price_a_usd IS NOT NULL
           ORDER BY q.bucket DESC
           LIMIT 1
      ) last_a ON true
      LEFT JOIN LATERAL (
          SELECT q.price_b_usd AS price
            FROM analytic_pool_prices_5min q
           WHERE q.pool_address = mp.pool_address AND q.bucket < tranche AND q.price_b_usd IS NOT NULL
           ORDER BY q.bucket DESC
           LIMIT 1
      ) last_b ON true
      LEFT JOIN pools_per_mint na ON na.mint = mp.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = mp.mint_b
     CROSS JOIN LATERAL (
         SELECT CASE WHEN na.pools > nb.pools THEN 'a' WHEN nb.pools > na.pools THEN 'b' END
                    AS anchor
     ) n
      LEFT JOIN LATERAL (SELECT yog_market_believed(mp.mint_a, q.price_usd) AS price_usd
                           FROM yog_price_asof(mp.mint_a, tranche + width) q) ma ON true
      LEFT JOIN LATERAL (SELECT yog_market_believed(mp.mint_b, q.price_usd) AS price_usd
                           FROM yog_price_asof(mp.mint_b, tranche + width) q) mb ON true
     WHERE NOT EXISTS (SELECT 1 FROM swapped s WHERE s.pool_address = mp.pool_address)
       -- A pool never priced has no bound at all: what it moved or paid stays unvalued.
       AND (last_a.price IS NOT NULL OR last_b.price IS NOT NULL)
)
SELECT * FROM swapped
UNION ALL
SELECT * FROM unswapped;
END;

