-- 024_price_snapshot_reserves.sql — pool_price_snapshot carries the pool's raw
-- reserves, so its reader can keep only the pools deep enough to compare.
-- Still raw inputs, no USD figure: the view stays exempt from the staleness policy.

-- ⚠️ Columns appended last: CREATE OR REPLACE VIEW refuses any other change.
CREATE OR REPLACE VIEW pool_price_snapshot AS
SELECT
    pcs.pool_address,
    pcs.protocol,
    pcs.last_sqrt_price,
    pcs.last_swap_at,
    tma.decimals   AS decimals_a,
    tmb.decimals   AS decimals_b,
    tpa.price_usd  AS price_a_usd,
    tpa.fetched_at AS price_a_fetched_at,
    tpb.price_usd  AS price_b_usd,
    tpb.fetched_at AS price_b_fetched_at,
    pcs.reserve_a,
    pcs.reserve_b
FROM pool_current_state pcs
JOIN pools p ON p.pool_address = pcs.pool_address
JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
JOIN token_last_price tpa ON tpa.mint = p.token_a_mint::TEXT
JOIN token_last_price tpb ON tpb.mint = p.token_b_mint::TEXT
WHERE pcs.last_sqrt_price IS NOT NULL
  AND pcs.last_swap_at IS NOT NULL;
