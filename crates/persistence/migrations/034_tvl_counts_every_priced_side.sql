-- 034_tvl_counts_every_priced_side.sql — a pool's TVL is the value of its priced
-- sides, a side without a price counting 0; NULL only when no side is valued.
-- tvl_complete_usd keeps the rule before it, for the readers a sub-total misleads.
-- The protocol TVL measured under that rule is deleted, and the pools first seen
-- without their birth observed get their own measure, per window.

-- ⚠️ LEFT JOINs: a pool without decimals stays, unvalued, instead of leaving the count.
CREATE OR REPLACE VIEW pool_current_tvl AS
SELECT
    sides.pool_address,
    COALESCE(sides.value_a_usd + sides.value_b_usd, sides.value_a_usd, sides.value_b_usd)
        AS tvl_usd,
    sides.value_a_usd,
    sides.value_b_usd,
    sides.value_a_usd + sides.value_b_usd AS tvl_complete_usd
FROM (
    SELECT
        pcs.pool_address,
        yog_amount_usd(pcs.reserve_a, tma.decimals, tpa.price_usd) AS value_a_usd,
        yog_amount_usd(pcs.reserve_b, tmb.decimals, tpb.price_usd) AS value_b_usd
    FROM pool_current_state pcs
    JOIN pools p ON p.pool_address = pcs.pool_address
    LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
    LEFT JOIN token_last_price tpa
           ON tpa.mint = p.token_a_mint::TEXT
          AND tpa.fetched_at >= now() - yog_price_max_age_latest()
    LEFT JOIN token_last_price tpb
           ON tpb.mint = p.token_b_mint::TEXT
          AND tpb.fetched_at >= now() - yog_price_max_age_latest()
) sides;

-- Not recomputable without the past reserves: the series starts again.
DELETE FROM analytic_protocol_tvl;

ALTER TABLE analytic_protocol_tvl ADD COLUMN pools_unvalued BIGINT NOT NULL;

-- `bucket` is the hour of the TVL row it splits; one row per window, 0 included.
CREATE TABLE analytic_protocol_tvl_discovered (
    protocol TEXT        NOT NULL,
    period   TEXT        NOT NULL CHECK (period IN ('24h', '7d', '30d')),
    bucket   TIMESTAMPTZ NOT NULL,
    tvl_usd  NUMERIC     NOT NULL,
    pools    BIGINT      NOT NULL,
    PRIMARY KEY (protocol, period, bucket)
);

GRANT SELECT, INSERT, UPDATE ON analytic_protocol_tvl_discovered TO yog_analytic;
GRANT SELECT ON analytic_protocol_tvl_discovered TO yog_api;
