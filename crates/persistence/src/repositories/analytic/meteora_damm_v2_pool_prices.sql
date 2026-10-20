WITH tranche AS (
    SELECT s.pool_address,
           p.token_a_mint::TEXT AS mint_a,
           p.token_b_mint::TEXT AS mint_b,
           tma.decimals AS dec_a,
           tmb.decimals AS dec_b
      FROM meteora_damm_v2_swap_events_5min s
      LEFT JOIN pools p ON p.pool_address = s.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
     WHERE s.bucket = $1
),
last_swap AS (
    SELECT DISTINCT ON (pool_address) pool_address, next_sqrt_price
      FROM meteora_damm_v2_swap_events
     WHERE timestamp >= $1 AND timestamp < $1::TIMESTAMPTZ + INTERVAL '5 minutes'
     ORDER BY pool_address, timestamp DESC, slot DESC,
              transaction_index DESC NULLS LAST, event_index DESC
),
pools_per_mint AS (
    SELECT m.mint, COUNT(*) AS pools
      FROM tranche
     CROSS JOIN LATERAL (VALUES (mint_a), (mint_b)) m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY m.mint
),
market AS (
    SELECT n.mint, pr.price_usd
      FROM pools_per_mint n
      LEFT JOIN LATERAL yog_price_asof(n.mint, $1::TIMESTAMPTZ + INTERVAL '5 minutes') pr ON true
),
quoted AS (
    SELECT t.pool_address,
           ma.price_usd AS market_a,
           mb.price_usd AS market_b,
           na.pools AS pools_a,
           nb.pools AS pools_b,
           -- ⚠️ Squared before the one division: dividing first loses the smallest prices.
           l.next_sqrt_price * l.next_sqrt_price / 2::NUMERIC ^ 128
               * 10::NUMERIC ^ (t.dec_a - t.dec_b) AS rate
      FROM tranche t
      LEFT JOIN last_swap l ON l.pool_address = t.pool_address
      LEFT JOIN pools_per_mint na ON na.mint = t.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = t.mint_b
      LEFT JOIN market ma ON ma.mint = t.mint_a
      LEFT JOIN market mb ON mb.mint = t.mint_b
)
INSERT INTO analytic_pool_prices_5min (
    pool_address, bucket, market_a_usd, price_a_usd, market_b_usd, price_b_usd
)
SELECT q.pool_address, $1, q.market_a, x.price_a, q.market_b, x.price_b
  FROM quoted q
 -- ⚠️ LEAST skips NULL: a token without a market price takes its price in the pool.
 CROSS JOIN LATERAL (
     SELECT CASE
                WHEN q.pools_a > q.pools_b THEN COALESCE(q.market_a, q.rate * q.market_b)
                WHEN q.pools_b > q.pools_a THEN
                    LEAST(q.market_a, q.rate * COALESCE(q.market_b, q.market_a / NULLIF(q.rate, 0)))
                ELSE LEAST(q.market_a, q.rate * q.market_b)
            END AS price_a,
            CASE
                WHEN q.pools_b > q.pools_a THEN COALESCE(q.market_b, q.market_a / NULLIF(q.rate, 0))
                WHEN q.pools_a > q.pools_b THEN
                    LEAST(q.market_b, COALESCE(q.market_a, q.rate * q.market_b) / NULLIF(q.rate, 0))
                ELSE LEAST(q.market_b, q.market_a / NULLIF(q.rate, 0))
            END AS price_b
 ) x
