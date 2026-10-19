-- 023_swap_hour_pricing.sql — the price of an hour of DAMM v2 swaps, written
-- once: the hourly priced view (007) and the analytic facts both call it.
-- The callers keep their joins to pools and token_metadata: inside the function
-- they would join each hour laterally, an index probe per hour, never a hash join.

-- ⚠️ Postgres inlines it only as one SELECT in LANGUAGE sql, neither STRICT nor
-- SECURITY DEFINER nor with a SET clause; otherwise it runs once per hour, unplanned.
CREATE FUNCTION meteora_damm_v2_swap_hour_prices(
    bucket      TIMESTAMPTZ,
    mint_a      TEXT,
    mint_b      TEXT,
    dec_a       SMALLINT,
    dec_b       SMALLINT,
    volume_in_a NUMERIC,
    volume_in_b NUMERIC,
    fee_in_a    NUMERIC,
    fee_in_b    NUMERIC,
    traded_a    NUMERIC,
    traded_b    NUMERIC
)
RETURNS TABLE (
    eff_price_a        NUMERIC,
    eff_price_b        NUMERIC,
    price_a_implied    BOOLEAN,
    price_b_implied    BOOLEAN,
    valuation_complete BOOLEAN
)
LANGUAGE sql STABLE PARALLEL SAFE
AS $$
SELECT
    e.eff_price_a,
    e.eff_price_b,
    (o.obs_a IS NULL AND i.implied_a IS NOT NULL),
    (o.obs_b IS NULL AND i.implied_b IS NOT NULL),
    NOT (
        (COALESCE(volume_in_a, 0) + COALESCE(fee_in_a, 0) > 0
            AND (e.eff_price_a IS NULL OR dec_a IS NULL))
        OR
        (COALESCE(volume_in_b, 0) + COALESCE(fee_in_b, 0) > 0
            AND (e.eff_price_b IS NULL OR dec_b IS NULL))
    )
FROM (SELECT) AS one
-- ⚠️ Qualified: in a SQL function a column of token_prices would shadow a parameter.
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = meteora_damm_v2_swap_hour_prices.mint_a
      AND fetched_at <= meteora_damm_v2_swap_hour_prices.bucket
      AND fetched_at >= meteora_damm_v2_swap_hour_prices.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = meteora_damm_v2_swap_hour_prices.mint_b
      AND fetched_at <= meteora_damm_v2_swap_hour_prices.bucket
      AND fetched_at >= meteora_damm_v2_swap_hour_prices.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true
CROSS JOIN LATERAL (
    SELECT NULLIF(pa.price_usd, 0) AS obs_a,
           NULLIF(pb.price_usd, 0) AS obs_b
) o
CROSS JOIN LATERAL (
    SELECT
        ((NULLIF(traded_b, 0)::NUMERIC / POWER(10::NUMERIC, dec_b)) * o.obs_b)
            / NULLIF(traded_a::NUMERIC / POWER(10::NUMERIC, dec_a), 0)
            AS implied_a,
        ((NULLIF(traded_a, 0)::NUMERIC / POWER(10::NUMERIC, dec_a)) * o.obs_a)
            / NULLIF(traded_b::NUMERIC / POWER(10::NUMERIC, dec_b), 0)
            AS implied_b
) i
CROSS JOIN LATERAL (
    SELECT COALESCE(o.obs_a, i.implied_a) AS eff_price_a,
           COALESCE(o.obs_b, i.implied_b) AS eff_price_b
) e
$$;

CREATE OR REPLACE VIEW meteora_damm_v2_swap_events_hourly_priced AS
SELECT
    h.pool_address,
    h.bucket,
    h.volume_in_a,
    h.volume_in_b,
    h.swap_count,
    h.fee_in_a,
    h.fee_in_b,
    h.protocol_fee_in_a,
    h.protocol_fee_in_b,
    h.referral_fee_in_a,
    h.referral_fee_in_b,
    tma.decimals AS dec_a,
    tmb.decimals AS dec_b,
    f.eff_price_a,
    f.eff_price_b,
    f.price_a_implied,
    f.price_b_implied,
    f.valuation_complete
FROM meteora_damm_v2_swap_events_hourly h
LEFT JOIN pools p ON p.pool_address = h.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
CROSS JOIN LATERAL meteora_damm_v2_swap_hour_prices(
    h.bucket, p.token_a_mint::TEXT, p.token_b_mint::TEXT, tma.decimals, tmb.decimals,
    h.volume_in_a, h.volume_in_b, h.fee_in_a, h.fee_in_b, h.traded_a, h.traded_b
) f;

-- ⚠️ The function reads token_prices as the caller, not as the view's owner.
GRANT SELECT ON token_prices TO yog_signals;
