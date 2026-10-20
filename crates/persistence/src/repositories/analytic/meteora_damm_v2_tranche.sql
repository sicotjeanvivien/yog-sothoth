-- ⚠️ One statement: the prices and the swaps then read the same snapshot of
-- the aggregate, and no pool can come in between them unpriced.
WITH tranche AS (
    SELECT s.*,
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
     WHERE timestamp >= $1 AND timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
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
      LEFT JOIN LATERAL yog_price_asof(n.mint, $1::TIMESTAMPTZ + $3::INTERVAL) pr ON true
),
quoted AS (
    SELECT t.pool_address,
           ma.price_usd AS market_a,
           mb.price_usd AS market_b,
           na.pools AS pools_a,
           nb.pools AS pools_b,
           -- ⚠️ Squared before the one division: dividing first loses the smallest prices.
           NULLIF(l.next_sqrt_price * l.next_sqrt_price / 2::NUMERIC ^ 128
                      * 10::NUMERIC ^ (t.dec_a - t.dec_b), 0) AS rate
      FROM tranche t
      LEFT JOIN last_swap l ON l.pool_address = t.pool_address
      LEFT JOIN pools_per_mint na ON na.mint = t.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = t.mint_b
      LEFT JOIN market ma ON ma.mint = t.mint_a
      LEFT JOIN market mb ON mb.mint = t.mint_b
),
prices AS (
    INSERT INTO analytic_pool_prices_5min (
        pool_address, bucket, market_a_usd, price_a_usd, market_b_usd, price_b_usd
    )
    SELECT q.pool_address, $1, q.market_a, x.price_a, q.market_b, x.price_b
      FROM quoted q
     -- ⚠️ LEAST skips NULL: a token without a market price takes its price in the pool.
     -- Without the pool's rate there is nothing to bound by, and no price but the anchor's.
     CROSS JOIN LATERAL (
         SELECT CASE
                    WHEN q.pools_a > q.pools_b THEN COALESCE(q.market_a, q.rate * q.market_b)
                    WHEN q.rate IS NULL THEN NULL
                    WHEN q.pools_b > q.pools_a THEN
                        LEAST(q.market_a, q.rate * COALESCE(q.market_b, q.market_a / q.rate))
                    ELSE LEAST(q.market_a, q.rate * q.market_b)
                END AS price_a,
                CASE
                    WHEN q.pools_b > q.pools_a THEN COALESCE(q.market_b, q.market_a / q.rate)
                    WHEN q.rate IS NULL THEN NULL
                    WHEN q.pools_a > q.pools_b THEN
                        LEAST(q.market_b, COALESCE(q.market_a, q.rate * q.market_b) / q.rate)
                    ELSE LEAST(q.market_b, q.market_a / q.rate)
                END AS price_b
     ) x
    RETURNING pool_address, price_a_usd, price_b_usd
)
INSERT INTO analytic_pool_5min (
    pool_address, bucket, protocol, swap_count, valued,
    volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
    protocol_fees_usd, referral_fees_usd
)
SELECT pool_address, $1, $2, swap_count, valued,
       volume_in_a_usd, volume_in_b_usd, fees_usd,
       fees_usd - protocol_fees_usd - referral_fees_usd,
       protocol_fees_usd, referral_fees_usd
  FROM (
    SELECT t.pool_address, t.swap_count, v.valued,
           CASE WHEN v.valued THEN yog_moved_amount_usd(t.volume_in_a, t.dec_a, pr.price_a_usd) END
               AS volume_in_a_usd,
           CASE WHEN v.valued THEN yog_moved_amount_usd(t.volume_in_b, t.dec_b, pr.price_b_usd) END
               AS volume_in_b_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(t.fee_in_a, t.dec_a, pr.price_a_usd)
             + yog_moved_amount_usd(t.fee_in_b, t.dec_b, pr.price_b_usd)
           END AS fees_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(t.protocol_fee_in_a, t.dec_a, pr.price_a_usd)
             + yog_moved_amount_usd(t.protocol_fee_in_b, t.dec_b, pr.price_b_usd)
           END AS protocol_fees_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(t.referral_fee_in_a, t.dec_a, pr.price_a_usd)
             + yog_moved_amount_usd(t.referral_fee_in_b, t.dec_b, pr.price_b_usd)
           END AS referral_fees_usd
      FROM tranche t
      JOIN prices pr ON pr.pool_address = t.pool_address
     CROSS JOIN LATERAL (
         SELECT NOT (
             (COALESCE(t.volume_in_a, 0) + COALESCE(t.fee_in_a, 0) > 0
                 AND (pr.price_a_usd IS NULL OR t.dec_a IS NULL))
             OR
             (COALESCE(t.volume_in_b, 0) + COALESCE(t.fee_in_b, 0) > 0
                 AND (pr.price_b_usd IS NULL OR t.dec_b IS NULL))
         ) AS valued
     ) v
  ) valued
