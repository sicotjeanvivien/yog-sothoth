-- ============================================================================
-- 006_flow_valuation_completeness.sql — "unknown" stops becoming "zero"
-- ============================================================================
-- Closes `.project` ticket 08. The two flow views that feed the Signal Engine
-- treated a missing price in opposite ways, and neither told the truth:
--
--                             | 023 → flow_imbalance | 025 → tvl_drain
--   price NULL                | ONE direction zeroes | BOTH directions zero
--   computed value            | (0 − X)/X = −1.0     | net_removed = 0
--   consequence               | guaranteed Critical  | pool skipped
--   failure mode              | SCREAMS              | STAYS SILENT
--
-- Same degraded input, opposite behaviours. Nobody decided that; it is the
-- difference between how the two views happen to be written.
--
-- ## Why now, and not "later, in its own PR"
--
-- Migration 005 made the loud half **reachable in the ordinary case**. Before
-- it, a leg only went NULL for a token that had never been priced at all; now it
-- goes NULL whenever a mint accumulates a price gap wider than
-- `yog_price_max_age_asof()`. Shipping 005 without this is shipping a known path
-- to a maximum-magnitude Critical on a balanced pool.
--
-- ## The decision, made once, for both views
--
-- **A flow whose window is not entirely valuable is UNKNOWN, not zero. An
-- unknown pool is skipped, and the skip is counted.**
--
-- That is already what `price_oracle_deviation` does (it requires both prices
-- and returns), and what `tvl_drain` achieves by accident through its `tvl_usd`
-- guard. Convergence goes toward that, never toward 023's behaviour.
--
-- ## ⚠️ Dropping the repositories' COALESCE is NOT enough on its own
--
-- `SUM` already skips NULLs, so a window where only *some* buckets are valuable
-- returns a **sub-total** with no NULL reaching the caller — the second defect
-- of the ticket, silent where the first one screams. The repositories therefore
-- aggregate a completeness flag ALONGSIDE the sum:
--
--     CASE WHEN bool_and(valuation_complete) THEN SUM(…) END
--
-- Complete window ⇒ no bucket is NULL ⇒ the sum is a true total. Otherwise NULL.
-- Publishing a sub-total becomes impossible rather than merely discouraged.
--
-- Both views expose the column under the SAME NAME so the two repositories read
-- the same word. The predicates necessarily differ — the amounts differ — but
-- the notion is named once per view and consumed identically.


-- ── meteora_damm_v2_pool_hourly_flow (023, now reading the priced view) ─────
-- 023 finally reads `meteora_damm_v2_swap_events_hourly_priced` instead of the
-- raw cagg. Not a tidy-up — it is what makes the de-COALESCE liveable, for two
-- reasons that are both measurable in the previous definition.
--
-- **The empty-leg defect, which 023 never got.** The old expression
-- `(COALESCE(volume_in_a,0) / 10^dec_a) * pa.price_usd` is NULL when
-- `volume_in_a = 0` and A's price is missing, because `0 * NULL` is NULL. A
-- one-way hour against an unpriced counter-token would therefore be declared
-- unvaluable when its value is *known, and is zero*. View 019's `swap_v` fixed
-- exactly this in 002 with `CASE WHEN <amount> = 0 THEN 0`; 023 kept the bug.
-- The distinction the CASE draws is the whole point:
--   * amount = 0             → contribute 0; nothing moved, its value is zero in
--                              any currency, known without any price;
--   * amount > 0, price NULL → the BUCKET is incomplete, and the outer guard
--                              turns both directions NULL together.
--
-- **The effective price** values the dominant shape (an unlisted memecoin
-- against SOL/USDC) through the rate its own swaps traded at, instead of
-- skipping it. Without it, "unknown ⇒ skip" would skip most of DAMM v2.
--
-- The two directions stay SEPARATE — that is 023's entire reason to exist, 019
-- sums them into one `volume_usd`.
--
-- ⚠️ Consequence: 023 no longer reads `token_prices` itself, so it leaves the
-- enumeration in `tests/price_staleness.rs`. It does not lose the staleness
-- bound — it inherits it from the priced view, which carries it.
DROP VIEW meteora_damm_v2_pool_hourly_flow;

CREATE VIEW meteora_damm_v2_pool_hourly_flow AS
SELECT
    h.pool_address,
    h.bucket,
    CASE WHEN NOT h.valuation_complete THEN NULL ELSE
        CASE WHEN COALESCE(h.volume_in_a, 0) = 0 THEN 0
             ELSE (h.volume_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
    END AS volume_a_to_b_usd,
    CASE WHEN NOT h.valuation_complete THEN NULL ELSE
        CASE WHEN COALESCE(h.volume_in_b, 0) = 0 THEN 0
             ELSE (h.volume_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
    END AS volume_b_to_a_usd,
    -- Carried so the repository can require the WHOLE window, not each bucket.
    h.valuation_complete
FROM meteora_damm_v2_swap_events_hourly_priced h;

GRANT SELECT ON meteora_damm_v2_pool_hourly_flow TO yog_signals;


-- ── meteora_damm_v2_pool_hourly_liquidity_flow (025, + valuation_complete) ──
-- The arithmetic does NOT change. The ticket is explicit that 025 is the view
-- that has it right: valuing both token legs of a direction is what makes its
-- failure symmetric, and 023 was to converge on it, never the reverse.
--
-- Only the flag is added. `added_usd IS NOT NULL` would carry the same
-- information here — NULL propagates across both legs, so a NULL value already
-- means "a price was missing" — but deriving the rule in one repository while
-- the other reads a named column is how a rule ends up applied at one site out
-- of two. The word is the same at both call sites, and it is spelled out here.
--
-- A side is REQUIRED when it carries any amount at all, in either direction; a
-- required side must have a price. Unlike the priced view of 002 there is no
-- decimals check: `pool_tokens` INNER-joins `token_metadata`, so a bucket whose
-- mints are unresolved never reaches this expression — it has already vanished.
-- That pre-existing asymmetry is left alone here.
DROP VIEW meteora_damm_v2_pool_hourly_liquidity_flow;

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
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true;

GRANT SELECT ON meteora_damm_v2_pool_hourly_liquidity_flow TO yog_signals;
