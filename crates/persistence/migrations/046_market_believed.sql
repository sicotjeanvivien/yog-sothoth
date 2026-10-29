-- 046_flow_price_witnessed.sql — a flow is never valued at a market price our pools contradict:
-- yog_price_asof gives no price where its latest observation is more than yog_market_over_pool_max()
-- times the token's ceiling (045), the rule `token_stock_price` holds a stock to.

-- ⚠️ The latest observation, then the check: an earlier one never stands in for a refused one.
CREATE OR REPLACE FUNCTION yog_price_asof(mint TEXT, at TIMESTAMPTZ)
RETURNS TABLE (price_usd NUMERIC)
LANGUAGE sql STABLE PARALLEL SAFE
BEGIN ATOMIC
    SELECT l.price_usd
      FROM (SELECT tp.price_usd FROM token_prices tp
             WHERE tp.mint = yog_price_asof.mint
               AND tp.fetched_at <= yog_price_asof.at
               AND tp.fetched_at >= yog_price_asof.at - yog_price_max_age_asof()
             ORDER BY tp.fetched_at DESC LIMIT 1) l
     WHERE NOT EXISTS (SELECT 1 FROM analytic_token_pool_ceilings c
                        WHERE c.mint = yog_price_asof.mint
                          AND l.price_usd > yog_market_over_pool_max() * c.ceiling_usd);
END;

-- ⚠️ Read as its caller: every role valuing through yog_price_asof reads the ceilings too.
GRANT SELECT ON analytic_token_pool_ceilings TO yog_api, yog_signals;
