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
-- What each side traded over its mean reserve, from the raw swaps: the aggregate keeps no reserve.
turnover AS (
    SELECT pool_address,
           SUM(amount_a)::NUMERIC / NULLIF(AVG(reserve_a_after), 0) AS rotation_a,
           SUM(amount_b)::NUMERIC / NULLIF(AVG(reserve_b_after), 0) AS rotation_b
      FROM meteora_damm_v2_swap_events
     WHERE timestamp >= $1 AND timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
     GROUP BY pool_address
),
pools_per_mint AS (
    SELECT m.mint, COUNT(*) AS pools
      FROM tranche
     CROSS JOIN LATERAL (VALUES (mint_a), (mint_b)) m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY m.mint
),
-- ⚠️ The believed price values the flows; the quote witnesses the partner, as it did in 045.
market AS (
    SELECT n.mint, yog_market_believed(n.mint, pr.price_usd) AS price_usd, pr.price_usd AS quote_usd
      FROM pools_per_mint n
      LEFT JOIN LATERAL yog_price_asof(n.mint, $1::TIMESTAMPTZ + $3::INTERVAL) pr ON true
),
quoted AS (
    SELECT t.pool_address, t.traded_a, t.traded_b, t.dec_a, t.dec_b,
           ma.price_usd AS market_a,
           mb.price_usd AS market_b,
           ma.quote_usd AS quote_a,
           mb.quote_usd AS quote_b,
           n.anchor,
           -- ⚠️ What the tranche traded, not the pool's mid price: on an empty pool one
           -- swap sends that to the end of its range.
           yog_token_amount(NULLIF(t.traded_b, 0), t.dec_b)
               / NULLIF(yog_token_amount(t.traded_a, t.dec_a), 0) AS rate,
           -- The token whose leg values the flows: the anchor, else the other, with a
           -- market price and decimals; both on a tie.
           CASE
               WHEN n.anchor = 'a' AND k.a_ok THEN 'a'
               WHEN n.anchor = 'b' AND k.b_ok THEN 'b'
               WHEN k.a_ok AND k.b_ok THEN 'both'
               WHEN k.a_ok THEN 'a'
               WHEN k.b_ok THEN 'b'
           END AS trust
      FROM tranche t
      LEFT JOIN pools_per_mint na ON na.mint = t.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = t.mint_b
      LEFT JOIN market ma ON ma.mint = t.mint_a
      LEFT JOIN market mb ON mb.mint = t.mint_b
     -- The anchor: the token traded in more pools of the tranche, none on a tie.
     CROSS JOIN LATERAL (
         SELECT CASE WHEN na.pools > nb.pools THEN 'a' WHEN nb.pools > na.pools THEN 'b' END
                    AS anchor
     ) n
     CROSS JOIN LATERAL (
         SELECT ma.price_usd IS NOT NULL AND t.dec_a IS NOT NULL AS a_ok,
                mb.price_usd IS NOT NULL AND t.dec_b IS NOT NULL AS b_ok
     ) k
),
prices AS (
    INSERT INTO analytic_pool_prices_5min (
        pool_address, bucket, market_a_usd, price_a_usd, market_b_usd, price_b_usd,
        pool_a_usd, pool_b_usd
    )
    SELECT q.pool_address, $1, q.market_a, x.price_a, q.market_b, x.price_b,
           CASE WHEN yog_amount_usd(q.traded_b, q.dec_b, q.quote_b) >= yog_pool_witness_min_usd()
                THEN q.rate * q.quote_b END,
           CASE WHEN yog_amount_usd(q.traded_a, q.dec_a, q.quote_a) >= yog_pool_witness_min_usd()
                THEN q.quote_a / q.rate END
      FROM quoted q
     -- ⚠️ LEAST skips NULL: a token without a market price takes its price in the pool.
     -- Without the tranche's rate there is nothing to bound by, and no price but the anchor's.
     CROSS JOIN LATERAL (
         SELECT CASE
                    WHEN q.anchor = 'a' THEN COALESCE(q.market_a, q.rate * q.market_b)
                    WHEN q.rate IS NULL THEN NULL
                    WHEN q.anchor = 'b' THEN
                        LEAST(q.market_a, q.rate * COALESCE(q.market_b, q.market_a / q.rate))
                    ELSE LEAST(q.market_a, q.rate * q.market_b)
                END AS price_a,
                CASE
                    WHEN q.anchor = 'b' THEN COALESCE(q.market_b, q.market_a / q.rate)
                    WHEN q.rate IS NULL THEN NULL
                    WHEN q.anchor = 'a' THEN
                        LEAST(q.market_b, COALESCE(q.market_a, q.rate * q.market_b) / q.rate)
                    ELSE LEAST(q.market_b, q.market_a / q.rate)
                END AS price_b
     ) x
)
INSERT INTO analytic_pool_5min (
    pool_address, bucket, protocol, swap_count, valued,
    volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
    protocol_fees_usd, referral_fees_usd, rotation
)
SELECT pool_address, $1, $2, swap_count, valued,
       volume_in_a_usd, volume_in_b_usd, fees_usd,
       fees_usd - protocol_fees_usd - referral_fees_usd,
       protocol_fees_usd, referral_fees_usd, rotation
  FROM (
    SELECT t.pool_address, t.swap_count, v.valued,
           -- ⚠️ Both sides quoted, or neither: the smaller, so the thinner side does not mark.
           -- A mint not resolved yet leaves the quoted side unknown: no rotation.
           CASE WHEN t.mint_a IS NULL OR t.mint_b IS NULL THEN NULL
                WHEN t.mint_a = ANY($4) AND NOT t.mint_b = ANY($4) THEN r.rotation_a
                WHEN t.mint_b = ANY($4) AND NOT t.mint_a = ANY($4) THEN r.rotation_b
                ELSE LEAST(r.rotation_a, r.rotation_b) END AS rotation,
           CASE WHEN v.valued THEN w.a_to_b END AS volume_in_a_usd,
           CASE WHEN v.valued THEN w.b_to_a END AS volume_in_b_usd,
           CASE WHEN v.valued THEN
               COALESCE(t.fee_in_a, 0) * u.per_a
             + sold.fee_b * u.sale_b + COALESCE(t.fee_in_b_on_buys, 0) * u.buy_b
           END AS fees_usd,
           CASE WHEN v.valued THEN
               COALESCE(t.protocol_fee_in_a, 0) * u.per_a
             + sold.protocol_b * u.sale_b + COALESCE(t.protocol_fee_in_b_on_buys, 0) * u.buy_b
           END AS protocol_fees_usd,
           CASE WHEN v.valued THEN
               COALESCE(t.referral_fee_in_a, 0) * u.per_a
             + sold.referral_b * u.sale_b + COALESCE(t.referral_fee_in_b_on_buys, 0) * u.buy_b
           END AS referral_fees_usd
      FROM tranche t
      JOIN quoted q ON q.pool_address = t.pool_address
      LEFT JOIN turnover r ON r.pool_address = t.pool_address
     -- ⚠️ A→B always pays its fee on the output, in B; B→A pays it in A in a BothToken
     -- pool, else on its input in B: the sales' B fees are what the buys' leave.
     CROSS JOIN LATERAL (
         SELECT COALESCE(t.fee_in_b, 0) - COALESCE(t.fee_in_b_on_buys, 0) AS fee_b,
                COALESCE(t.protocol_fee_in_b, 0) - COALESCE(t.protocol_fee_in_b_on_buys, 0)
                    AS protocol_b,
                COALESCE(t.referral_fee_in_b, 0) - COALESCE(t.referral_fee_in_b_on_buys, 0)
                    AS referral_b
     ) sold
     -- Each direction's gross output in raw units: its output, and the fee taken on it.
     CROSS JOIN LATERAL (
         SELECT t.traded_a - COALESCE(t.volume_in_a, 0) + COALESCE(t.fee_in_a, 0) AS out_a,
                t.traded_b - COALESCE(t.volume_in_b, 0) + sold.fee_b AS out_b
     ) o
     -- Each direction's gross at market price, by either leg: an output leg gets back the
     -- fee taken on it, and a B→A output the one its input paid in B.
     CROSS JOIN LATERAL (
         SELECT yog_moved_amount_usd(t.volume_in_a, t.dec_a, q.market_a) AS a_to_b_by_a,
                yog_moved_amount_usd(o.out_b, t.dec_b, q.market_b) AS a_to_b_by_b,
                yog_moved_amount_usd(t.volume_in_b, t.dec_b, q.market_b) AS b_to_a_by_b,
                -- ⚠️ Multiplied before the division: dividing first rounds the input's share.
                -- Nothing out — a dust input taken whole as fee — is worth nothing.
                CASE WHEN COALESCE(t.volume_in_b, 0) = 0 OR o.out_a = 0 THEN 0
                     ELSE yog_moved_amount_usd(o.out_a, t.dec_a, q.market_a)
                          * t.volume_in_b
                          / NULLIF(t.volume_in_b - COALESCE(t.fee_in_b_on_buys, 0), 0)
                END AS b_to_a_by_a
     ) g
     CROSS JOIN LATERAL (
         SELECT CASE q.trust
                    WHEN 'a' THEN g.a_to_b_by_a
                    WHEN 'b' THEN g.a_to_b_by_b
                    WHEN 'both' THEN LEAST(g.a_to_b_by_a, g.a_to_b_by_b)
                END AS a_to_b,
                CASE q.trust
                    WHEN 'b' THEN g.b_to_a_by_b
                    WHEN 'a' THEN g.b_to_a_by_a
                    WHEN 'both' THEN LEAST(g.b_to_a_by_b, g.b_to_a_by_a)
                END AS b_to_a
     ) w
     -- USD per raw unit of each fee: in a trusted token at its market price, otherwise
     -- at its share of the gross of the direction that paid it — A only on the buys.
     CROSS JOIN LATERAL (
         SELECT CASE
                    WHEN COALESCE(t.fee_in_a, 0) = 0 THEN 0
                    WHEN q.trust = 'b' THEN g.b_to_a_by_b / NULLIF(o.out_a, 0)
                    ELSE yog_amount_usd(1, t.dec_a, q.market_a)
                END AS per_a,
                CASE
                    WHEN sold.fee_b = 0 THEN 0
                    WHEN q.trust = 'a' THEN g.a_to_b_by_a / NULLIF(o.out_b, 0)
                    ELSE yog_amount_usd(1, t.dec_b, q.market_b)
                END AS sale_b,
                CASE
                    WHEN COALESCE(t.fee_in_b_on_buys, 0) = 0 THEN 0
                    WHEN q.trust = 'a' THEN g.b_to_a_by_a / NULLIF(t.volume_in_b, 0)
                    ELSE yog_amount_usd(1, t.dec_b, q.market_b)
                END AS buy_b
     ) u
     CROSS JOIN LATERAL (
         SELECT NOT (
             q.trust IS NULL OR w.a_to_b IS NULL OR w.b_to_a IS NULL
             OR u.per_a IS NULL OR u.sale_b IS NULL OR u.buy_b IS NULL
         ) AS valued
     ) v
  ) valued
