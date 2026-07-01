-- ============================================================================
-- 023 — meteora_damm_v2_pool_hourly_flow (VIEW)
-- ============================================================================
-- Directional per-(pool, hour) USD swap volume, feeding the Signal Engine's
-- flow-imbalance detector. Same trade-time, as-of-bucket valuation as VIEW
-- 019's `swap_v` CTE (raw cagg amount / 10^decimals, priced at the most recent
-- token_prices row as-of the bucket) — but the two trade directions are kept
-- SEPARATE. VIEW 019 sums them into a single `volume_usd`; a flow imbalance
-- needs each side on its own:
--   volume_a_to_b_usd = a_to_b input (token A, per the cagg's INPUT-side rule)
--                       valued at token A's price as-of the bucket
--   volume_b_to_a_usd = b_to_a input (token B) valued at token B's price
--
-- A separate, single-purpose view rather than more columns on 019: 019's
-- contract is the four-CA activity roll-up (combined volume); this stays
-- focused. It reuses the same pool_tokens decimals join (INNER → a pool whose
-- mints aren't resolved yet drops out, no row).
--
-- Definer's-rights view (owner = yog_migrate): the reading role needs only
-- SELECT on the view, not on the underlying cagg / token_prices.
-- ============================================================================

CREATE VIEW meteora_damm_v2_pool_hourly_flow AS
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
    (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
        AS volume_a_to_b_usd,
    (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS volume_b_to_a_usd
FROM meteora_damm_v2_swap_events_hourly h
JOIN pool_tokens pt ON pt.pool_address = h.pool_address
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true;

GRANT SELECT ON meteora_damm_v2_pool_hourly_flow TO yog_signals;
