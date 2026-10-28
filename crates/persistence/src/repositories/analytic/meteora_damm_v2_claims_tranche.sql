-- ⚠️ After the tranche's swaps: a pool that swapped in it takes the price just written.
WITH fee_claims AS (
    SELECT c.signature, c.event_index, c.timestamp, c.pool_address, c.owner,
           c.fee_a_claimed AS amount_a, c.fee_b_claimed AS amount_b,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
      FROM meteora_damm_v2_claim_position_fee_events c
      LEFT JOIN pools p ON p.pool_address = c.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
     WHERE c.timestamp >= $1 AND c.timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
),
reward_claims AS (
    SELECT c.signature, c.event_index, c.timestamp, c.pool_address, c.owner, c.mint_reward,
           c.total_reward AS amount,
           CASE WHEN c.mint_reward = p.token_a_mint::TEXT THEN 'a'
                WHEN c.mint_reward = p.token_b_mint::TEXT THEN 'b' END AS side,
           p.token_a_mint IS NOT NULL AND p.token_b_mint IS NOT NULL AS resolved,
           tm.decimals AS dec
      FROM meteora_damm_v2_claim_reward_events c
      LEFT JOIN pools p ON p.pool_address = c.pool_address
      LEFT JOIN token_metadata tm ON tm.mint = c.mint_reward
     WHERE c.timestamp >= $1 AND c.timestamp < $1::TIMESTAMPTZ + $3::INTERVAL
),
prices AS (
    SELECT * FROM meteora_damm_v2_tranche_pool_prices(
        $1, $3,
        ARRAY(SELECT pool_address FROM fee_claims
              UNION
              SELECT pool_address FROM reward_claims WHERE side IS NOT NULL))
),
fees_written AS (
    INSERT INTO analytic_position_fee_claims (
        signature, event_index, timestamp, bucket, pool_address, protocol, owner, amount_usd
    )
    SELECT f.signature, f.event_index, f.timestamp, $1, f.pool_address, $2, f.owner,
           -- An empty claim took nothing: worth 0, priced or not.
           CASE WHEN f.amount_a = 0 AND f.amount_b = 0 THEN 0 WHEN v.priced THEN v.a + v.b END
      FROM fee_claims f
      LEFT JOIN prices pr ON pr.pool_address = f.pool_address
     -- ⚠️ As a liquidity move: a side without a market price counts 0, and a side claimed with
     -- one needs its bounded price.
     CROSS JOIN LATERAL (
         SELECT CASE WHEN f.amount_a = 0 OR pr.market_a_usd IS NULL THEN 0
                     WHEN pr.price_a_usd IS NOT NULL AND f.dec_a IS NOT NULL
                     THEN yog_token_amount(f.amount_a::NUMERIC, f.dec_a) * pr.price_a_usd END AS a,
                CASE WHEN f.amount_b = 0 OR pr.market_b_usd IS NULL THEN 0
                     WHEN pr.price_b_usd IS NOT NULL AND f.dec_b IS NOT NULL
                     THEN yog_token_amount(f.amount_b::NUMERIC, f.dec_b) * pr.price_b_usd END AS b,
                (f.amount_a > 0 AND pr.market_a_usd IS NOT NULL)
                    OR (f.amount_b > 0 AND pr.market_b_usd IS NOT NULL) AS priced
     ) v
)
INSERT INTO analytic_reward_claims (
    signature, event_index, timestamp, bucket, pool_address, protocol, owner, mint_reward, amount_usd
)
SELECT r.signature, r.event_index, r.timestamp, $1, r.pool_address, $2, r.owner, r.mint_reward,
       CASE WHEN r.amount = 0 THEN 0
            WHEN r.dec IS NOT NULL AND v.price IS NOT NULL
            THEN yog_token_amount(r.amount::NUMERIC, r.dec) * v.price END
  FROM reward_claims r
  LEFT JOIN prices pr ON pr.pool_address = r.pool_address
  LEFT JOIN LATERAL yog_price_asof(r.mint_reward, $1::TIMESTAMPTZ + $3::INTERVAL) market
         ON r.side IS NULL AND r.resolved
 -- A token of the pool at the pool's bounded price, only with a market price as a move's side;
 -- any other at its market price. ⚠️ A pool's mints unresolved: the reward may be one, unvalued.
 CROSS JOIN LATERAL (
     SELECT CASE r.side
                WHEN 'a' THEN CASE WHEN pr.market_a_usd IS NOT NULL THEN pr.price_a_usd END
                WHEN 'b' THEN CASE WHEN pr.market_b_usd IS NOT NULL THEN pr.price_b_usd END
                ELSE market.price_usd
            END AS price
 ) v
