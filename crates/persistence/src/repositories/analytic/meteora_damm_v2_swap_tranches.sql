INSERT INTO analytic_pool_5min (
    pool_address, bucket, protocol, swap_count, valued,
    volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
    protocol_fees_usd, referral_fees_usd
)
SELECT pool_address, bucket, $2, swap_count, valued,
       volume_in_a_usd, volume_in_b_usd, fees_usd,
       fees_usd - protocol_fees_usd - referral_fees_usd,
       protocol_fees_usd, referral_fees_usd
  FROM (
    SELECT s.pool_address, s.bucket, s.swap_count, v.valued,
           CASE WHEN v.valued THEN yog_moved_amount_usd(s.volume_in_a, tma.decimals, pr.price_a_usd) END
               AS volume_in_a_usd,
           CASE WHEN v.valued THEN yog_moved_amount_usd(s.volume_in_b, tmb.decimals, pr.price_b_usd) END
               AS volume_in_b_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(s.fee_in_a, tma.decimals, pr.price_a_usd)
             + yog_moved_amount_usd(s.fee_in_b, tmb.decimals, pr.price_b_usd)
           END AS fees_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(s.protocol_fee_in_a, tma.decimals, pr.price_a_usd)
             + yog_moved_amount_usd(s.protocol_fee_in_b, tmb.decimals, pr.price_b_usd)
           END AS protocol_fees_usd,
           CASE WHEN v.valued THEN
               yog_moved_amount_usd(s.referral_fee_in_a, tma.decimals, pr.price_a_usd)
             + yog_moved_amount_usd(s.referral_fee_in_b, tmb.decimals, pr.price_b_usd)
           END AS referral_fees_usd
      FROM meteora_damm_v2_swap_events_5min s
      JOIN analytic_pool_prices_5min pr ON pr.pool_address = s.pool_address AND pr.bucket = s.bucket
      LEFT JOIN pools p ON p.pool_address = s.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
     CROSS JOIN LATERAL (
         SELECT NOT (
             (COALESCE(s.volume_in_a, 0) + COALESCE(s.fee_in_a, 0) > 0
                 AND (pr.price_a_usd IS NULL OR tma.decimals IS NULL))
             OR
             (COALESCE(s.volume_in_b, 0) + COALESCE(s.fee_in_b, 0) > 0
                 AND (pr.price_b_usd IS NULL OR tmb.decimals IS NULL))
         ) AS valued
     ) v
     WHERE s.bucket = $1
  ) valued
