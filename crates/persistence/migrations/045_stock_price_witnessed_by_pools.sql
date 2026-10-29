-- 045_stock_price_witnessed_by_pools.sql — a stock is valued at `token_stock_price`: the market
-- price, unless it is more than yog_market_over_pool_max() times the highest price our pools'
-- trades worth yog_pool_witness_min_usd() gave the token over yog_pool_witness_window(). Each
-- tranche keeps what its trade implies for each token; yog-analytic keeps each token's ceiling. The
-- DAMM v2 TVL hours measured under a price so contradicted are deleted.

CREATE FUNCTION yog_market_over_pool_max() RETURNS NUMERIC
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT 10::NUMERIC $$;

-- ⚠️ Within the 7-day retention of `analytic_pool_prices_5min` (028): a wider window reads no further.
CREATE FUNCTION yog_pool_witness_window() RETURNS INTERVAL
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT INTERVAL '7 days' $$;

-- What a trade must have paid for a token, at market, to witness its price: dust rounds its rate.
CREATE FUNCTION yog_pool_witness_min_usd() RETURNS NUMERIC
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT 1::NUMERIC $$;

-- ⚠️ Not `price_*_usd`: an anchor's is its own market price, and would vouch for itself.
-- The tranche's rate at the other token's market price; NULL without either, or under the minimum.
ALTER TABLE analytic_pool_prices_5min
    ADD COLUMN pool_a_usd NUMERIC,
    ADD COLUMN pool_b_usd NUMERIC;

-- ⚠️ CREATE TABLE AS, not UPDATE … FROM the aggregate: under DML its watermark is computed again
-- for every row it reads.
CREATE TABLE migration_045_traded AS
SELECT bucket, pool_address, traded_a, traded_b
  FROM meteora_damm_v2_swap_events_5min
 WHERE bucket >= (SELECT min(bucket) FROM analytic_pool_prices_5min)
   AND traded_a > 0 AND traded_b > 0;

UPDATE analytic_pool_prices_5min q
   SET pool_a_usd = w.pool_a_usd, pool_b_usd = w.pool_b_usd
  FROM (SELECT x.pool_address, x.bucket,
               CASE WHEN r.paid_b >= yog_pool_witness_min_usd() THEN r.rate * x.market_b_usd END AS pool_a_usd,
               CASE WHEN r.paid_a >= yog_pool_witness_min_usd() THEN x.market_a_usd / r.rate END AS pool_b_usd
          FROM analytic_pool_prices_5min x
          JOIN migration_045_traded t ON t.pool_address = x.pool_address AND t.bucket = x.bucket
          JOIN pools p ON p.pool_address = x.pool_address
          LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
          LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
         CROSS JOIN LATERAL (
             SELECT yog_token_amount(t.traded_b, tb.decimals)
                        / yog_token_amount(t.traded_a, ta.decimals) AS rate,
                    yog_amount_usd(t.traded_a, ta.decimals, x.market_a_usd) AS paid_a,
                    yog_amount_usd(t.traded_b, tb.decimals, x.market_b_usd) AS paid_b
         ) r
         WHERE r.paid_a >= yog_pool_witness_min_usd() OR r.paid_b >= yog_pool_witness_min_usd()) w
 WHERE q.pool_address = w.pool_address AND q.bucket = w.bucket;

DROP TABLE migration_045_traded;

CREATE TABLE analytic_token_pool_ceilings (
    mint        TEXT    PRIMARY KEY,
    ceiling_usd NUMERIC NOT NULL
);

INSERT INTO analytic_token_pool_ceilings (mint, ceiling_usd)
SELECT m.mint, max(m.price)
  FROM analytic_pool_prices_5min q
  JOIN pools p ON p.pool_address = q.pool_address
 CROSS JOIN LATERAL (VALUES (p.token_a_mint::TEXT, q.pool_a_usd),
                            (p.token_b_mint::TEXT, q.pool_b_usd)) m (mint, price)
 WHERE q.bucket >= now() - yog_pool_witness_window()
   AND m.mint IS NOT NULL AND m.price IS NOT NULL
 GROUP BY m.mint;

GRANT SELECT, INSERT, UPDATE, DELETE ON analytic_token_pool_ceilings TO yog_analytic;

-- A token no pool of ours traded within the window keeps its market price.
CREATE VIEW token_stock_price AS
SELECT tp.mint, tp.price_usd, tp.fetched_at
  FROM token_last_price tp
 WHERE tp.fetched_at >= now() - yog_price_max_age_latest()
   AND NOT EXISTS (SELECT 1 FROM analytic_token_pool_ceilings c
                    WHERE c.mint = tp.mint
                      AND tp.price_usd > yog_market_over_pool_max() * c.ceiling_usd);

-- ⚠️ LEFT JOINs: a pool without decimals stays, unvalued, instead of leaving the count.
CREATE OR REPLACE VIEW pool_current_tvl AS
SELECT
    sides.pool_address,
    COALESCE(sides.value_a_usd + sides.value_b_usd, sides.value_a_usd, sides.value_b_usd)
        AS tvl_usd,
    sides.value_a_usd,
    sides.value_b_usd,
    sides.value_a_usd + sides.value_b_usd AS tvl_complete_usd
FROM (
    SELECT
        pcs.pool_address,
        yog_amount_usd(pcs.reserve_a, tma.decimals, tpa.price_usd) AS value_a_usd,
        yog_amount_usd(pcs.reserve_b, tmb.decimals, tpb.price_usd) AS value_b_usd
    FROM pool_current_state pcs
    JOIN pools p ON p.pool_address = pcs.pool_address
    LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
    LEFT JOIN token_stock_price tpa ON tpa.mint = p.token_a_mint::TEXT
    LEFT JOIN token_stock_price tpb ON tpb.mint = p.token_b_mint::TEXT
) sides;

-- Measured under a price our pools contradict: false, not unmeasured. Only the hours more than
-- five times the last one before it; a fresh database has none.
DELETE FROM analytic_protocol_tvl_discovered d
 USING analytic_protocol_tvl t
 WHERE t.protocol = d.protocol AND t.bucket = d.bucket
   AND t.protocol = 'meteora_damm_v2' AND t.bucket >= '2026-10-29 06:00+00'
   AND t.tvl_usd > 5 * (SELECT tvl_usd FROM analytic_protocol_tvl
                         WHERE protocol = 'meteora_damm_v2' AND bucket = '2026-10-29 05:00+00');
DELETE FROM analytic_protocol_tvl
 WHERE protocol = 'meteora_damm_v2' AND bucket >= '2026-10-29 06:00+00'
   AND tvl_usd > 5 * (SELECT tvl_usd FROM analytic_protocol_tvl
                       WHERE protocol = 'meteora_damm_v2' AND bucket = '2026-10-29 05:00+00');
