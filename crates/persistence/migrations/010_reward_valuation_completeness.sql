-- ============================================================================
-- 010_reward_valuation_completeness.sql — a reward total is right, or absent
-- ============================================================================
-- Closes `.project` ticket 02. `reward_v`, inside
-- `meteora_damm_v2_pool_hourly_activity`, published a **sub-total that looked
-- like a total** on `GET /api/pools/{address}/history`, and nothing said so.
--
-- Its aggregate runs ACROSS the reward mints of one `(pool, bucket)` — the
-- underlying aggregate `meteora_damm_v2_claim_reward_events_hourly` groups by
-- `mint_reward` as well (baseline §13), so a bucket holds one row PER MINT.
-- `SUM` skips NULLs, so an unpriced mint next to a priced one yielded the
-- priced one's value under the name of the pair's total:
--
--     SELECT SUM(v) FROM (VALUES (100.0::numeric), (NULL::numeric)) t(v);  -- 100.0
--
-- This is, to the word, the defect 006's header describes under "⚠️ Dropping
-- the repositories' COALESCE is NOT enough on its own" — one CTE short of the
-- two it fixed. The rule was decided by ticket 08 and applied to the two views
-- the Signal Engine reads (023, 025); this view feeds the API path and never
-- got it. Applying a rule at one site out of two is how it got here, so it is
-- applied here in full rather than in the narrow shape that would close the
-- reported symptom.
--
-- ## Measured before writing it, 20 August 2026
--
-- The dev database cannot show this defect: 0 `claim_reward_events` (3
-- position-fee claims, last swap 12 August). It is demonstrated by reading the
-- SQL and reproduced in SQL, never observed in our own data — so the population
-- was measured on mainnet instead, over the whole cp-amm program:
--
--     cp-amm pools with reward_infos[0].initialized = 1 ....... 435
--     … of which reward_infos[1].initialized = 1 ..............   2
--
-- (`getProgramAccountsV2`, `dataSize = 1112`, memcmp at data offsets 728 and
-- 920 — `Pool.reward_infos` sits at 720 in the struct, each `RewardInfo` is 192
-- bytes. The offset was VERIFIED, not assumed: every mint decoded at +16
-- resolves to a real SPL Mint account, and an uninitialised slot carries the
-- null key.)
--
-- So a multi-mint reward bucket is reachable on **2 pools out of all of cp-amm**
-- today. That is the honest magnitude: this is a dormant defect, like the zero
-- price of 009, not one that is bleeding. It is closed because it traverses a
-- shipped guard — and because the population it needs is created by anyone who
-- funds a second reward on any of the 435.
--
-- ⚠️ **The empty leg is NOT bounded by that count.** It fires on single-mint
-- buckets too: a claim of 0 on an unpriced mint yields `0 * NULL = NULL` today,
-- declaring unknown a value that is known and is zero, in any currency. That
-- half is the one 002 fixed for `swap_v` and 006 for `liq_v` / `pos_fee_v`.
--
-- ## Flag, not coverage counter — and why that is free HERE
--
-- Two answers hold, and they do not say the same thing. A **flag** (`bool_and`,
-- as ticket 08) makes the figure vanish when it is not certain; a **coverage
-- counter** keeps it and says "covers 18 hours out of 24" (the shape of
-- `swap_buckets_priced_24h` / `swap_buckets_24h`). The general position is
-- counter for `/history`, flag for anything feeding a ranking or a threshold.
--
-- ⚠️ By that position the volume RANKING should carry one, and it carries
-- neither: `top_pool_addresses(PoolRankMetric::Volume24h)` orders on
-- `SUM(volume_usd)` with `HAVING SUM(volume_usd) IS NOT NULL`, so a pool
-- priceable 6 hours out of 24 is ranked on its 6-hour sub-total against a
-- neighbour's full 24. That is `.project` ticket 03 — 34 active pools out of 93
-- invisible, 5 August 2026, on a window the ticket itself calls too short to be
-- stable — and its fix is decided: MARK thin coverage in the dashboard rather
-- than change the order, the API already serialising the counters it needs
-- (`swapBuckets24h` / `swapBucketsPriced24h`). Named here, left alone.
--
-- For these columns the tension does not exist: **no screen renders them** —
-- `web/src/lib/api/schema/pool-history.ts` parses `rewardsClaimedUsd` and no
-- component reads it — so the flag costs no display at all. The day the
-- dashboard shows them, the counter argument becomes the right one again, and
-- it is `.project` that carries that discussion, not this header.
--
-- ## Two mechanisms, each at its own level
--
--   * the empty-leg `CASE`, INSIDE the `SUM`, per row: a mint that paid out
--     nothing contributes zero, and needs no price to do so;
--   * `bool_and`, AROUND it: one mint that DID pay out and cannot be valued
--     makes the whole bucket NULL.
--
-- Neither replaces the other. Without the `CASE`, a zero claim poisons a bucket
-- that is fully known; without `bool_and`, `SUM` silently drops the mint it
-- cannot value and the sub-total comes back.
--
-- ⚠️ **The `bool_and` argument is total, and the fix depends on it** — the same
-- load-bearing property 006 wrote down for the flow views. `bool_and` IGNORES
-- NULLs, so one NULL among TRUEs would aggregate to TRUE and re-publish the
-- sub-total, the fix defeating itself. Every operand of the predicate is either
-- an `IS NOT NULL` test or `COALESCE`-d, so it cannot be NULL.
--
-- **No `NULLIF(…, 0)`**: the `token_prices_price_usd_positive` constraint of
-- 009 makes a stored zero impossible, and that migration's header asks
-- explicitly for none to be added.
--
-- ## The INNER JOIN went too, and that is part of the same defect
--
-- `reward_v` joined `token_metadata` INNER, so a reward mint whose decimals
-- `yog-context` has not resolved yet **lost its row entirely** — and a row that
-- is gone is a row `bool_and` never sees. The sub-total would have come back
-- through that door, on the very buckets the guard was added to protect.
--
-- `crates/persistence/README.md` already states the rule ("Three ways to say we
-- don't know") and why `meteora_damm_v2_swap_events_hourly_priced` LEFT-joins
-- metadata: the bucket stays, unvaluable, instead of vanishing. `swap_v` has
-- it (`… OR tma.decimals IS NULL` in its `valuation_complete`); `reward_v` did
-- not. One site out of two, again.
--
-- ⚠️ Consequence, deliberate: `buckets` now yields `(pool, hour)` pairs whose
-- only activity is a claim on an unresolved mint. `/history` shows one more
-- hour, with NULL USD columns — "it was claimed, we could not price it" rather
-- than "nothing happened", which is exactly the change 002 made for swap
-- buckets on unresolved pool mints.
--
-- ## ⚠️ `liq_v` and `pos_fee_v` still have no completeness flag
--
-- **Any window aggregation over `liquidity_added_usd`, `liquidity_removed_usd`
-- or `fees_claimed_usd` must first give those CTEs a `valuation_complete`, the
-- way `swap_v` has one.** They got the empty-leg `CASE` in 006 and nothing
-- else, so a bucket is honest but a SUM over several is not.
--
-- Not fixed here, on purpose: no consumer aggregates them today —
-- `batch_compute` sums volume and the fee shares only, and `history` returns
-- them per bucket, where a NULL reads correctly as "unknown for this hour". A
-- flag nothing reads is a guard no test can make bite, and this repository has
-- paid that price already. The warning lives next to the definition rather than
-- in a tracker, which is the shape finding 12 settled on.
--
-- ## ⚠️ Two frozen headers now describe this CTE wrongly
--
-- `006_flow_valuation_completeness.sql:241-248` names the gap it was leaving —
-- *"`reward_v` is worse: its `SUM` aggregates ACROSS reward mints inside the
-- view … it needs its own flag or its own counter, and its own ticket"* — and
-- `007_referral_fee_split.sql:221-225` carries it forward as still true. It is
-- true of `liq_v` and `pos_fee_v`; it stopped being true of `reward_v` here.
-- Forward-only means neither file can be corrected in place, so the correction
-- lives in `migrations/README.md` (*"A frozen comment can go stale"*), and this
-- paragraph is the other end of that pointer.
--
-- ## Cost
--
-- A view redefinition, nothing else. No continuous aggregate is dropped or
-- rebuilt, so the free-rebuild window `migrations/README.md` dates to the first
-- production scheduler run is NOT spent here. Nothing depends on this view, so
-- the DROP takes no dependents with it.
-- ============================================================================


DROP VIEW meteora_damm_v2_pool_hourly_activity;


-- ── meteora_damm_v2_pool_hourly_activity (019, 002, 005, 006, 007, + reward) ─
-- `pool_tokens`, `swap_parts`, `swap_v`, `liq_v`, `pos_fee_v`, `buckets` and
-- the final SELECT are restored VERBATIM from 007 — read its header for the
-- fee split, and 006's for the empty legs. `reward_v` is the only change.
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
-- One row PER REWARD MINT per bucket, hence the guard: see the header for why
-- the empty leg sits inside the SUM and the completeness test outside it, and
-- why the metadata join is LEFT.
reward_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN bool_and((pr.price_usd IS NOT NULL AND tmr.decimals IS NOT NULL)
                           OR COALESCE(h.total_reward, 0) = 0)
             THEN SUM(CASE WHEN COALESCE(h.total_reward, 0) = 0 THEN 0
                           ELSE (h.total_reward::NUMERIC / POWER(10::NUMERIC, tmr.decimals)) * pr.price_usd END)
        END AS rewards_claimed_usd
    FROM meteora_damm_v2_claim_reward_events_hourly h
    LEFT JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
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
