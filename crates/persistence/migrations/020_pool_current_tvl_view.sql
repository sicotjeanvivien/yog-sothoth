-- ============================================================================
-- 020 — pool_current_tvl (VIEW)
-- ============================================================================
-- A read-time VIEW that encapsulates the per-pool *current* TVL valuation in
-- USD. It exists to DRY a valuation that was duplicated in Rust string SQL:
-- before this, the `reserve × most-recent-price` math (the `LATERAL
-- token_prices` + `POWER(10::NUMERIC, decimals)` joins) was copy-pasted in
-- BOTH `pool_analytics.batch_compute` (the per-pool `tvl_per_pool` CTE) and
-- the new `global_analytics` roll-up. Both now read this single view; their
-- Rust queries collapse to a trivial SELECT that the sqlx macro still verifies
-- against the view's columns.
--
-- Mirrors migration 019 (`meteora_damm_v2_pool_hourly_activity`) in spirit, but
-- is NOT protocol-prefixed: it reads only protocol-neutral tables
-- (`pool_current_state`, `pools`, `token_metadata`, `token_prices`), unlike 019
-- which reads the Meteora continuous aggregates.
--
-- Valuation mirrors the previous inline logic exactly: current reserves divided
-- by 10^decimals and priced at the most recent `token_prices` row (current
-- price, NOT as-of a bucket — this is a live snapshot, not history). A pool
-- whose mints aren't resolved drops out (INNER JOIN on token_metadata) → no
-- row. `tvl_usd` is NULL when either token has no known price (the arithmetic
-- propagates NULL), which is exactly what the partial-coverage callers expect:
-- the priced-pool count is `COUNT(*) FILTER (WHERE tvl_usd IS NOT NULL)`.
--
-- Not parameterized (a VIEW can't take args): it values every pool. Callers
-- filter with `WHERE pool_address = ANY(...)` or aggregate over the whole set.

CREATE VIEW pool_current_tvl AS
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
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

GRANT SELECT ON pool_current_tvl TO yog_api;
