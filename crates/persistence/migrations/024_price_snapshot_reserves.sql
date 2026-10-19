-- 024_price_snapshot_reserves.sql — the USD value of a raw token amount, written
-- once as yog_amount_usd and called by pool_current_tvl; pool_price_snapshot
-- carries the pool's raw reserves, so its reader values them through it too.
-- Still raw inputs, no USD figure: the view stays exempt from the staleness policy.

-- ⚠️ Inlined by Postgres only as one SELECT in LANGUAGE sql, neither STRICT nor
-- SECURITY DEFINER nor with a SET clause.
CREATE FUNCTION yog_amount_usd(amount NUMERIC, decimals INTEGER, price_usd NUMERIC)
RETURNS NUMERIC
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$ SELECT (amount / POWER(10::NUMERIC, decimals)) * price_usd $$;

CREATE OR REPLACE VIEW pool_current_tvl AS
SELECT
    sides.pool_address,
    sides.value_a_usd + sides.value_b_usd AS tvl_usd,
    sides.value_a_usd,
    sides.value_b_usd
FROM (
    SELECT
        pcs.pool_address,
        yog_amount_usd(pcs.reserve_a, tma.decimals, tpa.price_usd) AS value_a_usd,
        yog_amount_usd(pcs.reserve_b, tmb.decimals, tpb.price_usd) AS value_b_usd
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
