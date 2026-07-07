-- ============================================================================
-- 025 — meteora_damm_v2_pool_hourly_liquidity_flow (VIEW)
-- ============================================================================
-- Per-(pool, hour) USD liquidity flow, split by direction (added / removed),
-- feeding the Signal Engine's TVL-drain detector. Same trade-time,
-- as-of-bucket valuation as VIEW 023 (raw cagg amount / 10^decimals, priced
-- at the most recent token_prices row as-of the bucket), over the liquidity
-- cagg (011) instead of the swap cagg.
--
-- Each direction sums BOTH token legs (an add/remove touches both sides
-- together): added_usd = amount_a_added × price_a + amount_b_added × price_b,
-- same for removed_usd. A missing price as-of the bucket propagates NULL
-- through the whole expression — deliberate: a partially-priced flow would
-- silently undervalue the drain ratio, and the detector's TVL guard skips
-- unpriced pools anyway (pool_current_tvl is NULL for them too), so both
-- sides of the comparison go absent together rather than half-fake.
--
-- A separate, single-purpose view rather than more columns on 019: same
-- reasoning as 023 — 019's contract is the four-CA activity roll-up; this
-- stays focused on the directional liquidity flow. It reuses the same
-- pool_tokens decimals join (INNER → a pool whose mints aren't resolved yet
-- drops out, no row).
--
-- Definer's-rights view (owner = yog_migrate): the reading role needs only
-- SELECT on the view, not on the underlying cagg / token_prices.
-- ============================================================================

CREATE VIEW meteora_damm_v2_pool_hourly_liquidity_flow AS
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
    (COALESCE(h.amount_a_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
  + (COALESCE(h.amount_b_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS added_usd,
    (COALESCE(h.amount_a_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
  + (COALESCE(h.amount_b_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS removed_usd
FROM meteora_damm_v2_liquidity_events_hourly h
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

GRANT SELECT ON meteora_damm_v2_pool_hourly_liquidity_flow TO yog_signals;

-- The detector joins the flow with the pool's current TVL. `pool_current_tvl`
-- (migration 020) predates the yog_signals role's default privileges on
-- deployments provisioned before the signal engine existed, and the house
-- rule is explicit anyway: SELECT on an existing read-source is granted in
-- the migration that introduces the need for it.
GRANT SELECT ON pool_current_tvl TO yog_signals;
