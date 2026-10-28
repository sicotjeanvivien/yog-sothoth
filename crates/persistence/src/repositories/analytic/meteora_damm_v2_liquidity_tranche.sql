-- ⚠️ After the tranche's swaps: a pool that swapped in it takes the price just written.
-- A pool's creation deposits its first liquidity without a liquidity event: it is a move too.
WITH raw_moves AS (
    SELECT signature, event_index, timestamp, pool_address, owner,
           liquidity_event_kind AS kind, amount_a, amount_b
      FROM meteora_damm_v2_liquidity_events
     WHERE timestamp >= $1 AND timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
    UNION ALL
    SELECT signature, event_index, timestamp, pool_address, creator,
           'create', token_a_amount, token_b_amount
      FROM meteora_damm_v2_initialize_pool_events
     WHERE timestamp >= $1 AND timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
),
moves AS (
    SELECT e.*,
           p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
      FROM raw_moves e
      LEFT JOIN pools p ON p.pool_address = e.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
prices AS (
    SELECT * FROM meteora_damm_v2_tranche_pool_prices(
        $1, $3, ARRAY(SELECT DISTINCT pool_address FROM moves))
)
INSERT INTO analytic_liquidity_moves (
    signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind, amount_usd
)
SELECT m.signature, m.event_index, m.timestamp, $1, m.pool_address, $2, m.owner, m.kind,
       CASE WHEN v.priced THEN v.a + v.b END
  FROM moves m
  LEFT JOIN prices pr ON pr.pool_address = m.pool_address
 -- ⚠️ As the TVL: a token without a market price counts 0, never the price of one small trade
 -- spread over a whole stock. A side moved with a market price needs its bounded price.
 CROSS JOIN LATERAL (
     SELECT CASE WHEN m.amount_a = 0 OR pr.market_a_usd IS NULL THEN 0
                 WHEN pr.price_a_usd IS NOT NULL AND m.dec_a IS NOT NULL
                 THEN yog_token_amount(m.amount_a::NUMERIC, m.dec_a) * pr.price_a_usd END AS a,
            CASE WHEN m.amount_b = 0 OR pr.market_b_usd IS NULL THEN 0
                 WHEN pr.price_b_usd IS NOT NULL AND m.dec_b IS NOT NULL
                 THEN yog_token_amount(m.amount_b::NUMERIC, m.dec_b) * pr.price_b_usd END AS b,
            (m.amount_a > 0 AND pr.market_a_usd IS NOT NULL)
                OR (m.amount_b > 0 AND pr.market_b_usd IS NOT NULL) AS priced
 ) v
