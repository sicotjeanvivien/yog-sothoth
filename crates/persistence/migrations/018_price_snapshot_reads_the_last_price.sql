-- 018_price_snapshot_reads_the_last_price.sql — token_last_price completed with
-- the last price of every token in the series, and pool_price_snapshot joining
-- it.

-- ⚠️ No `WHERE`, unlike insert_batch's upsert: this sets each token to the
-- series' latest row, whatever the table held before.
INSERT INTO token_last_price (mint, price_usd, fetched_at)
SELECT DISTINCT ON (mint) mint, price_usd, fetched_at
  FROM token_prices
 ORDER BY mint, fetched_at DESC
ON CONFLICT (mint) DO UPDATE
   SET price_usd = EXCLUDED.price_usd, fetched_at = EXCLUDED.fetched_at;

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
