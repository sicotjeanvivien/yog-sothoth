-- ============================================================================
-- 024 — pool_price_snapshot (VIEW)
-- ============================================================================
-- Per-pool *current* price inputs from both sources, feeding the Signal
-- Engine's price-oracle-deviation detector: the on-chain side (the pool's
-- `last_sqrt_price`, to be decoded to a spot price in Rust — the Q64.64
-- interpretation is protocol-specific, so SQL only carries the raw value and
-- the `protocol` discriminator) and the oracle side (each token's most recent
-- `token_prices` row, with its `fetched_at` so the reader can gate on price
-- staleness). `last_swap_at` rides along for the symmetric gate: a pool that
-- has not traded recently has an equally stale spot price.
--
-- Like 020 (`pool_current_tvl`) this is NOT protocol-prefixed: it reads only
-- protocol-neutral tables (`pool_current_state`, `pools`, `token_metadata`,
-- `token_prices`). Like both 020 and 023 it is a live snapshot (latest price,
-- not as-of a bucket) and INNER joins drop pools that cannot be compared:
-- unresolved mints/decimals, no price observation for either token, or no
-- swap observed yet (`last_sqrt_price IS NULL`).
--
-- Definer's-rights view (owner = yog_migrate): the reading role needs only
-- SELECT on the view, not on the underlying tables.
-- ============================================================================

CREATE VIEW pool_price_snapshot AS
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
JOIN LATERAL (
    SELECT price_usd, fetched_at FROM token_prices
    WHERE mint = p.token_a_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
JOIN LATERAL (
    SELECT price_usd, fetched_at FROM token_prices
    WHERE mint = p.token_b_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true
WHERE pcs.last_sqrt_price IS NOT NULL
  AND pcs.last_swap_at IS NOT NULL;

GRANT SELECT ON pool_price_snapshot TO yog_signals;
