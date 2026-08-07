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
--
-- ⚠️ Two give-backs that come with reading the priced view, named rather than
-- glossed as a pure coverage gain:
--
--   * **the flag is defined over volume AND fees, 023 publishes only volume.**
--     The priced view requires a price for side X when
--     `volume_in_X + fee_in_X > 0`; 023 would only need one when
--     `volume_in_X > 0`. So a bucket carrying a fee but no volume on an unpriced
--     side NULLs both directions and `bool_and` then drops the pool, where the
--     answer was knowable. Measured over the full amount/price matrix: 608 of
--     2 916 combinations. Live incidence today is ZERO — every incomplete bucket
--     fails on its volume leg — so this is latent, and it turns real under a
--     `collect_fee_mode` charging the output token. A `volume_valuation_complete`
--     column on the priced view would remove it;
--   * **the row population changes.** Dropping the `pool_tokens` CTE means 023
--     inherits the priced view's LEFT JOINs: a pool with unresolved mints now
--     APPEARS, incomplete, and becomes a *counted* skip. View 025 keeps its
--     INNER JOIN, so the same pool vanishes from `tvl_drain`'s input entirely —
--     never considered, never skipped, invisible in both new counters. "The skip
--     is counted" therefore holds for `flow_imbalance` and not yet for
--     `tvl_drain`, which biases `skipped/considered` between the two. 0 pools
--     affected today (113 = 113 either way); left as-is rather than widened into
--     an unrelated change to 025's join semantics.
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
-- Two things are added: the flag, and the SAME empty-leg `CASE` the swap view
-- gets above. The second is not symmetry for its own sake — without it the
-- invariant this whole migration rests on does not hold on this side.
--
-- ⚠️ **`valuation_complete` must imply "no leg is NULL", and here it did not.**
-- A side is REQUIRED only when it carries an amount, but the value expression
-- still multiplies the untouched side: `(0 / 10^dec_b) * pb.price_usd`, and
-- `0 * NULL` is NULL. So a bucket where B moved nothing and B has no price came
-- out `valuation_complete = TRUE` with `added_usd = NULL` — `bool_and` stayed
-- true, `SUM` skipped the row, and the repository published the sub-total this
-- migration exists to make unrepresentable. Verified on a live database.
--
-- The shape is ordinary rather than exotic: a single-sided add or remove on a
-- concentrated position whose counter-token has no price in `[bucket-1h,
-- bucket]` — a state migration 005 turned from "never priced" into an everyday
-- one. `added_usd IS NOT NULL` is strictly STRONGER than `valuation_complete`,
-- and the weaker of the two was being used as the gate.
--
-- A side is REQUIRED when it carries any amount at all, in either direction; a
-- required side must have a price. Unlike the priced view of 002 there is no
-- decimals check: `pool_tokens` INNER-joins `token_metadata`, so a bucket whose
-- mints are unresolved never reaches this expression — it has already vanished.
-- That pre-existing asymmetry is left alone here.
--
-- What does NOT change is the arithmetic the ticket praises: a direction still
-- sums BOTH token legs, so a price missing on a side that actually moved still
-- annihilates both directions together. The `CASE` only stops a side that moved
-- NOTHING from annihilating them.
--
-- ⚠️ One door stays open here, deliberately: the flag tests `price_usd IS NULL`,
-- not `NULLIF(price_usd, 0)`. `token_prices.price_usd` is `NUMERIC(38,18)` with
-- no positivity CHECK and the Jupiter client writes it unfiltered, so a price
-- rounded to exactly zero yields `valuation_complete = TRUE` and a valuation of
-- 0 — "unknown" becoming "zero" through the one door left. The priced view of
-- 002 already defends with `NULLIF(…, 0)`; 020 and 021 share this gap and it
-- predates this migration. 0 such rows in 37 772 today. It belongs to
-- `.project/v01-prix-nul-hors-de-la-vue-des-swaps.md`, which explicitly asks not
-- to be folded into ticket 08 — so it is named here and left there.
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
    CASE WHEN COALESCE(h.amount_a_added, 0) = 0 THEN 0
         ELSE (h.amount_a_added::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd END
  + CASE WHEN COALESCE(h.amount_b_added, 0) = 0 THEN 0
         ELSE (h.amount_b_added::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd END
        AS added_usd,
    CASE WHEN COALESCE(h.amount_a_removed, 0) = 0 THEN 0
         ELSE (h.amount_a_removed::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd END
  + CASE WHEN COALESCE(h.amount_b_removed, 0) = 0 THEN 0
         ELSE (h.amount_b_removed::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd END
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


-- ── meteora_damm_v2_pool_hourly_activity (019, empty-leg fix on liq_v/pos_fee_v)
-- `liq_v` reads the SAME cagg, through the SAME two price LATERALs, to produce
-- the SAME number as `meteora_damm_v2_pool_hourly_liquidity_flow` above — one
-- for the API, one for the signal engine. Giving the empty-leg `CASE` to only
-- one of them would make them disagree: measured, 16 of 36 amount/price
-- combinations come out NULL here while the flow view values them, the
-- canonical case being an hour that adds only token A while token B has no
-- price in the window.
--
-- That is exactly the "one site out of two" shape this whole PR keeps removing,
-- and it would be one this PR *created*: before 006 neither view had the CASE.
-- `pos_fee_v` has the identical shape and gets the same treatment.
--
-- ⚠️ What this does NOT fix, and is left as a decision rather than smuggled in:
-- neither CTE carries a completeness flag, so `pool_analytics` can still sum a
-- partly-valuable window into a sub-total for these three figures — the API-side
-- twin of what 006 fixes for the detectors. `reward_v` is worse: its `SUM`
-- aggregates ACROSS reward mints inside the view, so one unpriced mint among two
-- already publishes a sub-total per bucket, with nothing to surface it (the
-- coverage counters are volume-only). Untouched here on purpose; it needs its
-- own flag or its own counter, and its own ticket.
DROP VIEW meteora_damm_v2_pool_hourly_activity;

CREATE VIEW meteora_damm_v2_pool_hourly_activity AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
swap_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            CASE WHEN COALESCE(h.volume_in_a, 0) = 0 THEN 0
                 ELSE (h.volume_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
          + CASE WHEN COALESCE(h.volume_in_b, 0) = 0 THEN 0
                 ELSE (h.volume_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
        END AS volume_usd,
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
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly_priced h
),
liq_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN COALESCE(h.amount_a_added, 0) = 0 THEN 0
             ELSE (h.amount_a_added::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd END
      + CASE WHEN COALESCE(h.amount_b_added, 0) = 0 THEN 0
             ELSE (h.amount_b_added::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd END
            AS liquidity_added_usd,
        CASE WHEN COALESCE(h.amount_a_removed, 0) = 0 THEN 0
             ELSE (h.amount_a_removed::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd END
      + CASE WHEN COALESCE(h.amount_b_removed, 0) = 0 THEN 0
             ELSE (h.amount_b_removed::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd END
            AS liquidity_removed_usd
    FROM meteora_damm_v2_liquidity_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
pos_fee_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN COALESCE(h.fee_a_claimed, 0) = 0 THEN 0
             ELSE (h.fee_a_claimed::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd END
      + CASE WHEN COALESCE(h.fee_b_claimed, 0) = 0 THEN 0
             ELSE (h.fee_b_claimed::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd END
            AS fees_claimed_usd
    FROM meteora_damm_v2_claim_position_fee_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
reward_v AS (
    SELECT h.pool_address, h.bucket,
        SUM((COALESCE(h.total_reward, 0)::NUMERIC / POWER(10::NUMERIC, tmr.decimals)) * pr.price_usd) AS rewards_claimed_usd
    FROM meteora_damm_v2_claim_reward_events_hourly h
    JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = h.mint_reward::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pr ON true
    GROUP BY h.pool_address, h.bucket
),
buckets AS (
    SELECT pool_address, bucket FROM swap_v
    UNION SELECT pool_address, bucket FROM liq_v
    UNION SELECT pool_address, bucket FROM pos_fee_v
    UNION SELECT pool_address, bucket FROM reward_v
)
SELECT
    b.pool_address,
    b.bucket,
    s.volume_usd,
    s.fees_usd,
    s.protocol_fees_usd,
    s.swap_count,
    l.liquidity_added_usd,
    l.liquidity_removed_usd,
    pf.fees_claimed_usd,
    rw.rewards_claimed_usd
FROM buckets b
LEFT JOIN swap_v s    ON s.pool_address = b.pool_address AND s.bucket = b.bucket
LEFT JOIN liq_v l     ON l.pool_address = b.pool_address AND l.bucket = b.bucket
LEFT JOIN pos_fee_v pf ON pf.pool_address = b.pool_address AND pf.bucket = b.bucket
LEFT JOIN reward_v rw  ON rw.pool_address = b.pool_address AND rw.bucket = b.bucket;

GRANT SELECT ON meteora_damm_v2_pool_hourly_activity TO yog_api;
