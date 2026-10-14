-- 017_token_last_price.sql — the last price written for each token, kept by the
-- statement that inserts into token_prices (`insert_batch`), so that
-- pool_current_tvl joins one row per token instead of searching the hypertable
-- twice per pool. Only yog_context writes here, by upsert.

CREATE TABLE token_last_price (
    mint       TEXT            PRIMARY KEY,
    price_usd  NUMERIC(38, 18) NOT NULL,
    fetched_at TIMESTAMPTZ     NOT NULL,
    CONSTRAINT token_last_price_price_usd_positive CHECK (price_usd > 0)
);

-- ⚠️ SELECT too: the upsert's `WHERE` reads the row it would replace.
GRANT SELECT, INSERT, UPDATE ON token_last_price TO yog_context;

INSERT INTO token_last_price (mint, price_usd, fetched_at)
SELECT DISTINCT ON (mint) mint, price_usd, fetched_at
  FROM token_prices
 WHERE fetched_at >= now() - yog_price_max_age_latest()
 ORDER BY mint, fetched_at DESC;

CREATE OR REPLACE VIEW pool_current_tvl AS
SELECT
    pcs.pool_address,
    (
        (pcs.reserve_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
      + (pcs.reserve_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
    ) AS tvl_usd
FROM pool_current_state pcs
JOIN pools p ON p.pool_address = pcs.pool_address
JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
LEFT JOIN token_last_price tpa
       ON tpa.mint = p.token_a_mint::TEXT
      AND tpa.fetched_at >= now() - yog_price_max_age_latest()
LEFT JOIN token_last_price tpb
       ON tpb.mint = p.token_b_mint::TEXT
      AND tpb.fetched_at >= now() - yog_price_max_age_latest();
