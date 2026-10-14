-- 018_price_snapshot_reads_the_last_price.sql — pool_price_snapshot joins one
-- row per token in token_last_price (017) instead of searching the hypertable
-- twice per pool. Same columns, same inner joins, still no bound on the
-- price's age: the view stays exempt from the staleness rule.

-- ⚠️ The last price since 017, not of the whole series: a token missing here has
-- no recent price, and price_oracle_deviation drops an old one in Rust anyway.
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
    tpb.fetched_at AS price_b_fetched_at
FROM pool_current_state pcs
JOIN pools p ON p.pool_address = pcs.pool_address
JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
JOIN token_last_price tpa ON tpa.mint = p.token_a_mint::TEXT
JOIN token_last_price tpb ON tpb.mint = p.token_b_mint::TEXT
WHERE pcs.last_sqrt_price IS NOT NULL
  AND pcs.last_swap_at IS NOT NULL;
