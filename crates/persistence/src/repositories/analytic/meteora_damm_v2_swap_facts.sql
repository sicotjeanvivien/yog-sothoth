WITH hours AS (
    SELECT time_bucket('1 hour', s.bucket) AS bucket,
           s.pool_address,
           SUM(s.volume_in_a)       AS volume_in_a,
           SUM(s.volume_in_b)       AS volume_in_b,
           SUM(s.swap_count)::BIGINT AS swap_count,
           SUM(s.fee_in_a)          AS fee_in_a,
           SUM(s.fee_in_b)          AS fee_in_b,
           SUM(s.protocol_fee_in_a) AS protocol_fee_in_a,
           SUM(s.protocol_fee_in_b) AS protocol_fee_in_b,
           SUM(s.referral_fee_in_a) AS referral_fee_in_a,
           SUM(s.referral_fee_in_b) AS referral_fee_in_b,
           SUM(s.traded_a)          AS traded_a,
           SUM(s.traded_b)          AS traded_b
      FROM meteora_damm_v2_swap_events_5min s
     -- ⚠️ On the tranche: on `time_bucket('1 hour', …)` it would not reach the chunk scans.
     WHERE s.bucket >= $1 AND s.bucket < $2::TIMESTAMPTZ + INTERVAL '1 hour'
     GROUP BY 1, 2
),
priced AS (
    SELECT h.*,
           tma.decimals AS dec_a,
           tmb.decimals AS dec_b,
           f.eff_price_a,
           f.eff_price_b,
           f.valuation_complete
      FROM hours h
      LEFT JOIN pools p ON p.pool_address = h.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
      CROSS JOIN LATERAL meteora_damm_v2_swap_hour_prices(
          h.bucket, p.token_a_mint::TEXT, p.token_b_mint::TEXT, tma.decimals, tmb.decimals,
          h.volume_in_a, h.volume_in_b, h.fee_in_a, h.fee_in_b, h.traded_a, h.traded_b
      ) f
)
INSERT INTO analytic_pool_hourly (
    pool_address, bucket, protocol, swap_count, valued,
    volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
    protocol_fees_usd, referral_fees_usd
)
SELECT pool_address, bucket, $3, swap_count, valued,
       volume_in_a_usd, volume_in_b_usd, fees_usd,
       fees_usd - protocol_fees_usd - referral_fees_usd,
       protocol_fees_usd, referral_fees_usd
  FROM (
    SELECT h.pool_address, h.bucket, h.swap_count,
           h.valuation_complete AS valued,
           CASE WHEN NOT h.valuation_complete THEN NULL
                WHEN COALESCE(h.volume_in_a, 0) = 0 THEN 0
                ELSE (h.volume_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a
           END AS volume_in_a_usd,
           CASE WHEN NOT h.valuation_complete THEN NULL
                WHEN COALESCE(h.volume_in_b, 0) = 0 THEN 0
                ELSE (h.volume_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b
           END AS volume_in_b_usd,
           CASE WHEN NOT h.valuation_complete THEN NULL ELSE
               CASE WHEN COALESCE(h.fee_in_a, 0) = 0 THEN 0
                    ELSE (h.fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
             + CASE WHEN COALESCE(h.fee_in_b, 0) = 0 THEN 0
                    ELSE (h.fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
           END AS fees_usd,
           CASE WHEN NOT h.valuation_complete THEN NULL ELSE
               CASE WHEN COALESCE(h.protocol_fee_in_a, 0) = 0 THEN 0
                    ELSE (h.protocol_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
             + CASE WHEN COALESCE(h.protocol_fee_in_b, 0) = 0 THEN 0
                    ELSE (h.protocol_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
           END AS protocol_fees_usd,
           CASE WHEN NOT h.valuation_complete THEN NULL ELSE
               CASE WHEN COALESCE(h.referral_fee_in_a, 0) = 0 THEN 0
                    ELSE (h.referral_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
             + CASE WHEN COALESCE(h.referral_fee_in_b, 0) = 0 THEN 0
                    ELSE (h.referral_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
           END AS referral_fees_usd
      FROM priced h
  ) valued
ON CONFLICT (pool_address, bucket) DO UPDATE SET
    protocol          = EXCLUDED.protocol,
    swap_count        = EXCLUDED.swap_count,
    valued            = EXCLUDED.valued,
    volume_in_a_usd   = EXCLUDED.volume_in_a_usd,
    volume_in_b_usd   = EXCLUDED.volume_in_b_usd,
    fees_usd          = EXCLUDED.fees_usd,
    lp_fees_usd       = EXCLUDED.lp_fees_usd,
    protocol_fees_usd = EXCLUDED.protocol_fees_usd,
    referral_fees_usd = EXCLUDED.referral_fees_usd
