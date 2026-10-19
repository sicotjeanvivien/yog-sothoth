-- 026_token_amount.sql — a raw amount in token units, written once as
-- yog_token_amount: yog_amount_usd values it, and the implied price of 023
-- divides by it.

CREATE FUNCTION yog_token_amount(amount NUMERIC, decimals INTEGER)
RETURNS NUMERIC
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$ SELECT amount / POWER(10::NUMERIC, decimals) $$;

CREATE OR REPLACE FUNCTION yog_amount_usd(amount NUMERIC, decimals INTEGER, price_usd NUMERIC)
RETURNS NUMERIC
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$ SELECT yog_token_amount(amount, decimals) * price_usd $$;

CREATE OR REPLACE FUNCTION meteora_damm_v2_swap_hour_prices(
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
        yog_amount_usd(NULLIF(traded_b, 0), dec_b, o.obs_b)
            / NULLIF(yog_token_amount(traded_a, dec_a), 0)
            AS implied_a,
        yog_amount_usd(NULLIF(traded_a, 0), dec_a, o.obs_a)
            / NULLIF(yog_token_amount(traded_b, dec_b), 0)
            AS implied_b
) i
CROSS JOIN LATERAL (
    SELECT COALESCE(o.obs_a, i.implied_a) AS eff_price_a,
           COALESCE(o.obs_b, i.implied_b) AS eff_price_b
) e
$$;
