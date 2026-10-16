-- 019_pool_side_values.sql — pool_current_tvl also publishes the USD value of
-- each side of a pool, under the same price bound as its TVL, which is their sum.

-- ⚠️ New columns go after tvl_usd: CREATE OR REPLACE only appends columns.
CREATE OR REPLACE VIEW pool_current_tvl AS
SELECT
    sides.pool_address,
    sides.value_a_usd + sides.value_b_usd AS tvl_usd,
    sides.value_a_usd,
    sides.value_b_usd
FROM (
    SELECT
        pcs.pool_address,
        (pcs.reserve_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd AS value_a_usd,
        (pcs.reserve_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd AS value_b_usd
    FROM pool_current_state pcs
    JOIN pools p ON p.pool_address = pcs.pool_address
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
    LEFT JOIN token_last_price tpa
           ON tpa.mint = p.token_a_mint::TEXT
          AND tpa.fetched_at >= now() - yog_price_max_age_latest()
    LEFT JOIN token_last_price tpb
           ON tpb.mint = p.token_b_mint::TEXT
          AND tpb.fetched_at >= now() - yog_price_max_age_latest()
) sides;
