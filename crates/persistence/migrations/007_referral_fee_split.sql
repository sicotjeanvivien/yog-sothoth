-- ============================================================================
-- 007_referral_fee_split.sql — the referral is Meteora's cut, not the LPs'
-- ============================================================================
-- Fixes the finding of `.project` ticket 05: `lpFees` is published as
-- `fees - protocol_fees`, which credits the referral to the liquidity
-- providers. cp-amm takes it out of the PROTOCOL share
-- (`cp-amm/src/state/fee.rs::split_fees`):
--
--     protocol_fee_brut = fee_amount × protocol_fee_percent / 100
--     trading_fee       = fee_amount − protocol_fee_brut
--     compounding_fee   = trading_fee × compounding_fee_bps / 10_000
--     claiming_fee      = trading_fee − compounding_fee
--     referral_fee      = protocol_fee_brut × referral_fee_percent / 100
--     protocol_fee      = protocol_fee_brut − referral_fee   ← what is emitted
--
-- So the LP share is `claiming + compounding`, and the published figure was
-- `claiming + compounding + referral`. The four components summing to the total
-- was never in question — the emitted `protocol_fee` is already net of the
-- referral, so `fee_in_*` double-counts nothing. Only the SPLIT was wrong.
--
-- Measured 4 August 2026 (relevé in ticket 00): the referral is 0,14 % to
-- 0,89 % of a pool's total fees, not the 4 % the default percents allow — the
-- large majority of swaps carry no referral account. ~0,2 % in value. The
-- figure is wrong by construction rather than by accident, which is why it is
-- fixed; the magnitude is stated so nobody later reads urgency into it.
--
-- ## Why this costs a cagg rebuild
--
-- `meteora_damm_v2_swap_events_hourly` sums the four components into
-- `fee_in_a` / `fee_in_b` and exposes `protocol_fee` alone, so `referral_fee`
-- cannot be recovered downstream: a read-side patch is impossible. The raw
-- column `meteora_damm_v2_swap_events.referral_fee` is untouched, so nothing
-- has to be re-ingested — the 30-day retention window is enough to
-- re-materialize.
--
-- 002's header carries the full argument for why a cagg is dropped and rebuilt
-- rather than doubled, and why that is still free today (no cagg has ever
-- materialized a bucket; the job scheduler has been off since 16 June —
-- ticket 03). It is not repeated here. What IS worth recording: this is the
-- SECOND rebuild of this aggregate — its third definition, across three
-- migrations (001 created it, 002 rebuilt it, this one rebuilds it again) — and
-- 002 already had the window open on 5 August without adding them. `migrations/
-- README.md` dates the end of the free rebuild at the first scheduler run in
-- production, so the columns go in now rather than when they are needed.
--
-- ## Why `fee_in_*` keeps being the total of the four
--
-- The alternative — folding the referral into `protocol_fee_in_*` — needs no
-- new column, but it destroys the referral as a distinguishable quantity and
-- leaves a column whose name stops describing its content. `fee_in_*` as the
-- total is what Meteora's docs call the Trading Fee, it is the natural
-- denominator of `effectiveFeeBps`, and it keeps the three shares additive:
--
--     fees_usd = lp_fees_usd + protocol_fees_usd + referral_fees_usd
--
-- ⚠️ `001_baseline.sql` §13 — the header of this same aggregate, and the file
-- people read to learn an object's current shape — still says "the LP share is
-- (fee_in_x - protocol_fee_in_x)". That is the formula this migration removes,
-- and forward-only means it cannot be edited there. It is flagged in
-- `migrations/README.md`; this note is the other end of the pointer, for
-- whoever diffs the two definitions side by side.
--
-- ## Why the split is computed here and not in the DTO
--
-- It was computed in TWO presentation sites (`api/.../pool.rs` and
-- `.../pool_history.rs`), each with its own copy of `fees - protocol`. Fixing
-- it there means fixing the same definition twice — the "one site out of two"
-- shape 006's header names. In SQL it is written once, and the four figures are
-- already governed by a single `valuation_complete`, so `lp_fees_usd` is NULL
-- exactly when the other three are, for free.
--
-- ## What is dropped only because it depends on the aggregate
--
-- `meteora_damm_v2_swap_events_hourly_priced` gains the two columns as
-- pass-through; `meteora_damm_v2_pool_hourly_flow` is restored verbatim (it
-- reads neither the referral nor the shares). `liq_v` / `pos_fee_v` /
-- `reward_v` inside the activity view are restored verbatim too, including the
-- missing-completeness-flag caveat 006 records against them — that is its own
-- ticket, not this one's.
-- ============================================================================


-- ── drop the dependents, innermost last ─────────────────────────────────────
DROP VIEW meteora_damm_v2_pool_hourly_activity;
DROP VIEW meteora_damm_v2_pool_hourly_flow;
DROP VIEW meteora_damm_v2_swap_events_hourly_priced;
DROP MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly;


-- ── swap volume + realized fees, rebuilt  [was §13, 010/014/017, then 002] ──
-- Unchanged from 002 except for the last two columns. `referral_fee_in_a` /
-- `referral_fee_in_b` are direction-filtered on `fee_token_is_a` exactly like
-- `fee_in_*` and `protocol_fee_in_*`: the fee of a swap is charged in ONE of
-- the two tokens, and which one is a pool-level mode, so a bucket can hold fees
-- in both only if the pool's mode changed inside the hour.
CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                        AS bucket,
    pool_address,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE fee_token_is_a)                       AS fee_in_a,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE NOT fee_token_is_a)                   AS fee_in_b,
    SUM(protocol_fee) FILTER (WHERE fee_token_is_a)         AS protocol_fee_in_a,
    SUM(protocol_fee) FILTER (WHERE NOT fee_token_is_a)     AS protocol_fee_in_b,
    SUM(referral_fee) FILTER (WHERE fee_token_is_a)         AS referral_fee_in_a,
    SUM(referral_fee) FILTER (WHERE NOT fee_token_is_a)     AS referral_fee_in_b,
    SUM(amount_a)                                           AS traded_a,
    SUM(amount_b)                                           AS traded_b
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_swap_events_hourly TO yog_api;


-- ── meteora_damm_v2_swap_events_hourly_priced (002, 005, + referral) ────────
-- Unchanged from 005 but for the two pass-through columns. See 002 for why
-- `token_metadata` and `pools` are LEFT-joined, why every zero is `NULLIF`-ed,
-- why this view carries the cagg's own columns instead of just the prices, and
-- why it needs no explicit GRANT.
--
-- ⚠️ `valuation_complete` is deliberately NOT widened to mention the referral,
-- and that is not an oversight. It asks "does this side carry an amount that
-- needs a price", and `referral_fee_in_x` is a SUBSET of `fee_in_x` — a side
-- carrying a referral already carries a fee, so the condition is unchanged.
-- `protocol_fee_in_x` is absent from it for the same reason.
CREATE VIEW meteora_damm_v2_swap_events_hourly_priced AS
SELECT
    h.pool_address,
    h.bucket,
    h.volume_in_a,
    h.volume_in_b,
    h.swap_count,
    h.fee_in_a,
    h.fee_in_b,
    h.protocol_fee_in_a,
    h.protocol_fee_in_b,
    h.referral_fee_in_a,
    h.referral_fee_in_b,
    tma.decimals AS dec_a,
    tmb.decimals AS dec_b,
    e.eff_price_a,
    e.eff_price_b,
    (o.obs_a IS NULL AND i.implied_a IS NOT NULL) AS price_a_implied,
    (o.obs_b IS NULL AND i.implied_b IS NOT NULL) AS price_b_implied,
    NOT (
        (COALESCE(h.volume_in_a, 0) + COALESCE(h.fee_in_a, 0) > 0
            AND (e.eff_price_a IS NULL OR tma.decimals IS NULL))
        OR
        (COALESCE(h.volume_in_b, 0) + COALESCE(h.fee_in_b, 0) > 0
            AND (e.eff_price_b IS NULL OR tmb.decimals IS NULL))
    ) AS valuation_complete
FROM meteora_damm_v2_swap_events_hourly h
LEFT JOIN pools p ON p.pool_address = h.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint::TEXT AND fetched_at <= h.bucket
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint::TEXT AND fetched_at <= h.bucket
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true
CROSS JOIN LATERAL (
    SELECT NULLIF(pa.price_usd, 0) AS obs_a,
           NULLIF(pb.price_usd, 0) AS obs_b
) o
CROSS JOIN LATERAL (
    SELECT
        ((NULLIF(h.traded_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * o.obs_b)
            / NULLIF(h.traded_a::NUMERIC / POWER(10::NUMERIC, tma.decimals), 0)
            AS implied_a,
        ((NULLIF(h.traded_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * o.obs_a)
            / NULLIF(h.traded_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals), 0)
            AS implied_b
) i
CROSS JOIN LATERAL (
    SELECT COALESCE(o.obs_a, i.implied_a) AS eff_price_a,
           COALESCE(o.obs_b, i.implied_b) AS eff_price_b
) e;


-- ── meteora_damm_v2_pool_hourly_flow (023, 005, 006) ────────────────────────
-- Restored verbatim from 006. It reads neither the fee shares nor the referral;
-- it is dropped only because it selects from the priced view above.
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


-- ── meteora_damm_v2_pool_hourly_activity (019, 002, 005, 006, + the split) ──
-- `liq_v`, `pos_fee_v` and `reward_v` are restored verbatim from 006, caveats
-- included: none of them carries a completeness flag, so `pool_analytics` can
-- still sum a partly-valuable window into a sub-total for those three figures.
-- 006's header states it; it is still true, it is still someone else's ticket,
-- and it is still not smuggled in here.
--
-- `swap_v` is the one that changes, and it is split in two levels. Postgres
-- cannot reference an output alias from the same SELECT list, so deriving the
-- LP share in one level would mean writing the fee and protocol expressions a
-- second time — re-creating, inside the fix, the duplication the fix exists to
-- remove. `swap_parts` values the three MEASURED quantities, `swap_v` derives
-- the one that is a subtraction:
--
--     lp = fees − protocol − referral
--
-- Additive by construction: the three shares sum back to `fees_usd` exactly,
-- and they are NULL together (one `valuation_complete` governs all four).
CREATE VIEW meteora_damm_v2_pool_hourly_activity AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
swap_parts AS (
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
        CASE WHEN NOT h.valuation_complete THEN NULL ELSE
            CASE WHEN COALESCE(h.referral_fee_in_a, 0) = 0 THEN 0
                 ELSE (h.referral_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
          + CASE WHEN COALESCE(h.referral_fee_in_b, 0) = 0 THEN 0
                 ELSE (h.referral_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
        END AS referral_fees_usd,
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly_priced h
),
swap_v AS (
    SELECT pool_address, bucket, volume_usd, fees_usd,
           protocol_fees_usd, referral_fees_usd,
           fees_usd - protocol_fees_usd - referral_fees_usd AS lp_fees_usd,
           swap_count
    FROM swap_parts
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
    s.referral_fees_usd,
    s.lp_fees_usd,
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
