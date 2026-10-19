-- 027_price_asof.sql — the price of a token at an instant, written once as
-- yog_price_asof: the last observation at or before it, within
-- yog_price_max_age_asof(). The valued views and the hour pricing of 023 call it.

-- ⚠️ Inlined by Postgres only as one SELECT in LANGUAGE sql, neither STRICT nor
-- SECURITY DEFINER nor with a SET clause; it reads token_prices as the caller.
-- ⚠️ BEGIN ATOMIC, not a string: token_prices stays a dependency, bound at creation.
-- ⚠️ Qualified parameters: a column of token_prices would shadow them.
CREATE FUNCTION yog_price_asof(mint TEXT, at TIMESTAMPTZ)
RETURNS TABLE (price_usd NUMERIC)
LANGUAGE sql STABLE PARALLEL SAFE
BEGIN ATOMIC
    SELECT tp.price_usd FROM token_prices tp
    WHERE tp.mint = yog_price_asof.mint
      AND tp.fetched_at <= yog_price_asof.at
      AND tp.fetched_at >= yog_price_asof.at - yog_price_max_age_asof()
    ORDER BY tp.fetched_at DESC LIMIT 1;
END;

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
LEFT JOIN LATERAL yog_price_asof(mint_a, bucket) pa ON true
LEFT JOIN LATERAL yog_price_asof(mint_b, bucket) pb ON true
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

CREATE OR REPLACE VIEW meteora_damm_v2_pool_hourly_liquidity_flow AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
)
SELECT
    h.pool_address,
    h.bucket,
    yog_moved_amount_usd(h.amount_a_added, pt.dec_a, pa.price_usd)
  + yog_moved_amount_usd(h.amount_b_added, pt.dec_b, pb.price_usd)
        AS added_usd,
    yog_moved_amount_usd(h.amount_a_removed, pt.dec_a, pa.price_usd)
  + yog_moved_amount_usd(h.amount_b_removed, pt.dec_b, pb.price_usd)
        AS removed_usd,
    NOT (
        (COALESCE(h.amount_a_added, 0) + COALESCE(h.amount_a_removed, 0) > 0
            AND pa.price_usd IS NULL)
        OR
        (COALESCE(h.amount_b_added, 0) + COALESCE(h.amount_b_removed, 0) > 0
            AND pb.price_usd IS NULL)
    ) AS valuation_complete
FROM meteora_damm_v2_liquidity_events_hourly h
JOIN pool_tokens pt ON pt.pool_address = h.pool_address
LEFT JOIN LATERAL yog_price_asof(pt.token_a_mint, h.bucket) pa ON true
LEFT JOIN LATERAL yog_price_asof(pt.token_b_mint, h.bucket) pb ON true;

CREATE OR REPLACE VIEW meteora_damm_v2_pool_hourly_activity AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
swap_parts AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            yog_moved_amount_usd(h.volume_in_a, h.dec_a, h.eff_price_a)
          + yog_moved_amount_usd(h.volume_in_b, h.dec_b, h.eff_price_b)
        END AS volume_usd,
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            yog_moved_amount_usd(h.fee_in_a, h.dec_a, h.eff_price_a)
          + yog_moved_amount_usd(h.fee_in_b, h.dec_b, h.eff_price_b)
        END AS fees_usd,
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            yog_moved_amount_usd(h.protocol_fee_in_a, h.dec_a, h.eff_price_a)
          + yog_moved_amount_usd(h.protocol_fee_in_b, h.dec_b, h.eff_price_b)
        END AS protocol_fees_usd,
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            yog_moved_amount_usd(h.referral_fee_in_a, h.dec_a, h.eff_price_a)
          + yog_moved_amount_usd(h.referral_fee_in_b, h.dec_b, h.eff_price_b)
        END AS referral_fees_usd,
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly_priced h
),
swap_v AS (
    SELECT pool_address, bucket, volume_usd, fees_usd,
           protocol_fees_usd, referral_fees_usd,
           fees_usd - protocol_fees_usd - referral_fees_usd AS lp_fees_usd,
           swap_count
    FROM swap_parts
),
liq_v AS (
    SELECT h.pool_address, h.bucket,
        yog_moved_amount_usd(h.amount_a_added, pt.dec_a, pa.price_usd)
      + yog_moved_amount_usd(h.amount_b_added, pt.dec_b, pb.price_usd)
            AS liquidity_added_usd,
        yog_moved_amount_usd(h.amount_a_removed, pt.dec_a, pa.price_usd)
      + yog_moved_amount_usd(h.amount_b_removed, pt.dec_b, pb.price_usd)
            AS liquidity_removed_usd
    FROM meteora_damm_v2_liquidity_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL yog_price_asof(pt.token_a_mint, h.bucket) pa ON true
    LEFT JOIN LATERAL yog_price_asof(pt.token_b_mint, h.bucket) pb ON true
),
pos_fee_v AS (
    SELECT h.pool_address, h.bucket,
        yog_moved_amount_usd(h.fee_a_claimed, pt.dec_a, pa.price_usd)
      + yog_moved_amount_usd(h.fee_b_claimed, pt.dec_b, pb.price_usd)
            AS fees_claimed_usd
    FROM meteora_damm_v2_claim_position_fee_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL yog_price_asof(pt.token_a_mint, h.bucket) pa ON true
    LEFT JOIN LATERAL yog_price_asof(pt.token_b_mint, h.bucket) pb ON true
),
reward_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN bool_and((pr.price_usd IS NOT NULL AND tmr.decimals IS NOT NULL)
                           OR COALESCE(h.total_reward, 0) = 0)
             THEN SUM(yog_moved_amount_usd(h.total_reward, tmr.decimals, pr.price_usd))
        END AS rewards_claimed_usd
    FROM meteora_damm_v2_claim_reward_events_hourly h
    LEFT JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
    LEFT JOIN LATERAL yog_price_asof(h.mint_reward, h.bucket) pr ON true
    GROUP BY h.pool_address, h.bucket
),
buckets AS (
    SELECT pool_address, bucket FROM swap_v
    UNION SELECT pool_address, bucket FROM liq_v
    UNION SELECT pool_address, bucket FROM pos_fee_v
    UNION SELECT pool_address, bucket FROM reward_v
)
SELECT
    b.pool_address,
    b.bucket,
    s.volume_usd,
    s.fees_usd,
    s.protocol_fees_usd,
    s.referral_fees_usd,
    s.lp_fees_usd,
    s.swap_count,
    l.liquidity_added_usd,
    l.liquidity_removed_usd,
    pf.fees_claimed_usd,
    rw.rewards_claimed_usd
FROM buckets b
LEFT JOIN swap_v s    ON s.pool_address = b.pool_address AND s.bucket = b.bucket
LEFT JOIN liq_v l     ON l.pool_address = b.pool_address AND l.bucket = b.bucket
LEFT JOIN pos_fee_v pf ON pf.pool_address = b.pool_address AND pf.bucket = b.bucket
LEFT JOIN reward_v rw  ON rw.pool_address = b.pool_address AND rw.bucket = b.bucket;

CREATE OR REPLACE VIEW meteora_damm_v2_liquidity_events_valued AS
SELECT
    le.pool_address,
    le.signature,
    le.timestamp,
    le.liquidity_event_kind,
    le.amount_a,
    le.amount_b,
    le.liquidity_delta,
    le.reserve_a_after,
    le.reserve_b_after,
    le.position,
    le.owner,
    (
        yog_amount_usd(le.amount_a, tma.decimals, tpa.price_usd)
      + yog_amount_usd(le.amount_b, tmb.decimals, tpb.price_usd)
    ) AS value_usd,
    le.slot,
    le.event_index,
    le.transaction_index
FROM meteora_damm_v2_liquidity_events le
LEFT JOIN pools p ON p.pool_address = le.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint
LEFT JOIN LATERAL yog_price_asof(p.token_a_mint, le.timestamp) tpa ON true
LEFT JOIN LATERAL yog_price_asof(p.token_b_mint, le.timestamp) tpb ON true;
