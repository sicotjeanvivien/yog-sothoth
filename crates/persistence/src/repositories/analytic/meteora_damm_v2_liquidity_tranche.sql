-- ⚠️ After the tranche's swaps: a pool that swapped in it takes the price just written.
WITH moves AS (
    SELECT e.signature, e.event_index, e.timestamp, e.pool_address, e.owner,
           e.liquidity_event_kind AS kind, e.amount_a, e.amount_b,
           p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
      FROM meteora_damm_v2_liquidity_events e
      LEFT JOIN pools p ON p.pool_address = e.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
     WHERE e.timestamp >= $1 AND e.timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
),
moved_pools AS (
    SELECT DISTINCT pool_address, mint_a, mint_b FROM moves
),
-- The anchor, as for the swaps: the token in more pools of the tranche, none on a tie.
pools_per_mint AS (
    SELECT m.mint, COUNT(*) AS pools
      FROM (SELECT s.pool_address, p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b
              FROM meteora_damm_v2_swap_events_5min s
              JOIN pools p ON p.pool_address = s.pool_address
             WHERE s.bucket = $1
            UNION
            SELECT pool_address, mint_a, mint_b FROM moved_pools) t
     CROSS JOIN LATERAL (VALUES (t.mint_a), (t.mint_b)) m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY m.mint
),
swapped AS (
    SELECT q.pool_address, q.price_a_usd, q.price_b_usd
      FROM moved_pools mp
      JOIN analytic_pool_prices_5min q ON q.pool_address = mp.pool_address AND q.bucket = $1
),
-- Without a swap: the anchor at market, the other at the lower of its market and the pool's
-- last bounded price. ⚠️ Never the pool's mid price; without a last price, no value.
unswapped AS (
    SELECT mp.pool_address,
           CASE WHEN n.anchor = 'a' THEN COALESCE(ma.price_usd, last.price_a_usd)
                ELSE LEAST(ma.price_usd, last.price_a_usd) END AS price_a_usd,
           CASE WHEN n.anchor = 'b' THEN COALESCE(mb.price_usd, last.price_b_usd)
                ELSE LEAST(mb.price_usd, last.price_b_usd) END AS price_b_usd
      FROM moved_pools mp
     CROSS JOIN LATERAL (
         SELECT q.price_a_usd, q.price_b_usd
           FROM analytic_pool_prices_5min q
          WHERE q.pool_address = mp.pool_address AND q.bucket < $1
          ORDER BY q.bucket DESC
          LIMIT 1
     ) last
      LEFT JOIN pools_per_mint na ON na.mint = mp.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = mp.mint_b
     CROSS JOIN LATERAL (
         SELECT CASE WHEN na.pools > nb.pools THEN 'a' WHEN nb.pools > na.pools THEN 'b' END
                    AS anchor
     ) n
      LEFT JOIN LATERAL yog_price_asof(mp.mint_a, $1::TIMESTAMPTZ + $3::INTERVAL) ma ON true
      LEFT JOIN LATERAL yog_price_asof(mp.mint_b, $1::TIMESTAMPTZ + $3::INTERVAL) mb ON true
     WHERE NOT EXISTS (SELECT 1 FROM swapped s WHERE s.pool_address = mp.pool_address)
),
prices AS (
    SELECT * FROM swapped
    UNION ALL
    SELECT * FROM unswapped
)
INSERT INTO analytic_liquidity_moves (
    signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind, amount_usd
)
SELECT m.signature, m.event_index, m.timestamp, $1, m.pool_address, $2, m.owner, m.kind,
       -- A side moved needs its price and decimals; a side not moved is worth nothing.
       CASE WHEN (m.amount_a = 0 OR (pr.price_a_usd IS NOT NULL AND m.dec_a IS NOT NULL))
             AND (m.amount_b = 0 OR (pr.price_b_usd IS NOT NULL AND m.dec_b IS NOT NULL))
            THEN COALESCE(yog_token_amount(m.amount_a::NUMERIC, m.dec_a) * pr.price_a_usd, 0)
               + COALESCE(yog_token_amount(m.amount_b::NUMERIC, m.dec_b) * pr.price_b_usd, 0)
       END
  FROM moves m
  LEFT JOIN prices pr ON pr.pool_address = m.pool_address
