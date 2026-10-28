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
    signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind, lock, amount_usd
)
SELECT m.signature, m.event_index, m.timestamp, $1, m.pool_address, $2, m.owner, m.kind,
       -- ⚠️ Permanent first: a transaction locking both ways keeps the stronger lock. The tranche's
       -- constant bounds too: the planner excludes chunks on them, never on `m.timestamp`.
       CASE WHEN EXISTS (SELECT 1 FROM meteora_damm_v2_permanent_lock_position_events l
                          WHERE l.signature = m.signature AND l.timestamp = m.timestamp
                            AND l.pool_address = m.pool_address
                            AND l.timestamp >= $1 AND l.timestamp < $1::TIMESTAMPTZ + $3::INTERVAL)
            THEN 'permanent'
            WHEN EXISTS (SELECT 1 FROM meteora_damm_v2_lock_position_events l
                          WHERE l.signature = m.signature AND l.timestamp = m.timestamp
                            AND l.pool_address = m.pool_address
                            AND l.timestamp >= $1 AND l.timestamp < $1::TIMESTAMPTZ + $3::INTERVAL)
            THEN 'vesting'
            ELSE 'none' END,
       yog_pool_amounts_usd(m.amount_a, m.amount_b, m.dec_a, m.dec_b,
                            pr.price_a_usd, pr.price_b_usd, pr.market_a_usd, pr.market_b_usd)
  FROM moves m
  LEFT JOIN prices pr ON pr.pool_address = m.pool_address
