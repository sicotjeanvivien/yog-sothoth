-- ============================================================================
-- 005_price_staleness_policy.sql — a price observation has a validity window
-- ============================================================================
-- Until this migration there was NO policy on how old a price may be. Measured
-- on 7 August 2026: **7 views** read `token_prices` through **17 LATERAL
-- lookups**, and exactly **one** of them bounded the age — `pool_price_snapshot`
-- (024), which merely exposes `fetched_at` so that `price_oracle_deviation` can
-- gate in Rust (`max_price_age`, 15 min). The other sixteen took whatever the
-- most recent row happened to be, however old.
--
-- ## The failure that motivated it
--
-- `pool_current_tvl` (020) takes the latest known price with no time bound at
-- all, and it feeds BOTH the dashboard's TVL and the denominator + the
-- `min_tvl_usd` floor of the `tvl_drain` detector. Should `yog-context` stop,
-- `price_oracle_deviation` falls silent correctly — it has its guard — while the
-- dashboard keeps publishing a stale TVL and `tvl_drain` keeps computing ratios
-- against it. Nothing says so. That is a silent failure, and it is live today.
--
-- ## The two questions, which are not the same question
--
-- The lookups come in two shapes, and conflating them is how the bound gets
-- written wrong:
--
--   * **as-of** (019, 021, 023, 025, and the priced view of 002) — the LATERAL
--     asks `fetched_at <= <event or bucket time>`. The question is how old the
--     price was **relative to the thing being valued**. A bucket from ten days
--     ago valued by a price from ten days ago is CORRECT. Bounding it against
--     `now()` would collapse the whole history — that is the trap here.
--   * **latest** (020) — no time reference at all, the price is "current". The
--     question is how old it is **relative to now()**.
--
-- Hence two constants, not one: 1 hour for as-of (one bucket width — a price
-- more than a bucket away was not describing that hour), 15 minutes for latest
-- (the value `price_oracle_deviation` already settled on; the price worker
-- samples every 30 s, so 15 min is 30 missed ticks — wide enough to absorb a
-- Jupiter hiccup, tight enough to catch a real outage).
--
-- ## Why functions rather than literals
--
-- The rule was previously applied at one site out of seventeen. Spelling
-- `INTERVAL '1 hour'` sixteen times is how that happens again. The interval is
-- named once; changing the policy is one migration and one line.
--
-- `IMMUTABLE` matters: the call sits inside a LATERAL evaluated per row, and an
-- IMMUTABLE SQL function is inlined and constant-folded at planning time. The
-- bound then turns the open-ended backward scan on
-- `idx_token_prices_mint_recent (mint, fetched_at DESC)` into a bounded range.
--
-- No GRANT is needed: `setup_roles.sql` issues no `REVOKE ... FROM PUBLIC`, so
-- EXECUTE on a new function is already held by every role. Nothing to add to the
-- privilege matrix asserted by `tests/privileges.rs` (it compares explicit
-- grants only).
--
-- ## What this migration deliberately does NOT do
--
--   * **`pool_price_snapshot` (024) is untouched.** It is not a valuation view:
--     it produces no USD figure, it publishes raw inputs WITH their timestamps
--     precisely so the consumer can arbitrate — and the consumer does, paired
--     with `max_spot_age` on `last_swap_at`, a guard of a different nature.
--     Forcing it into SQL would break its `!` non-null overrides, make the skip
--     uncountable, and merge two unrelated gates. The line drawn here is: **the
--     policy binds valuation, not comparison.**
--   * **The left edge is not staleness.** A mint carries no price at all before
--     `yog-context` first knows it; the as-of LATERAL then finds nothing. That
--     is ABSENCE, not expiry, and it belongs to `.project` ticket 08 and the
--     effective price of 002. A staleness bound must not pretend to treat it.
--   * **View 023 keeps reading the swap cagg directly.** Making it read the
--     priced view is ticket 08's fix; it is not mixed in here.
--
-- ## The asymmetry this leaves standing, on purpose
--
-- `pool_current_tvl` is a product `reserves × price`. This bounds the price to
-- 15 minutes while the reserves, coming from `pool_current_state`, may be weeks
-- old (`.project` ticket 00, § 2). Bounding one input of a product is an
-- improvement, not a completion.
--
-- Every view below is reproduced verbatim from its previous definition; the only
-- change in each is the added `fetched_at >=` line. Column lists and their ORDER
-- are preserved — `meteora_damm_v2_liquidity_events_valued` is read by
-- `query_as!` calls that map BY POSITION (041), so a reordering would silently
-- mis-assign columns.


-- ── the policy, named once ──────────────────────────────────────────────────

CREATE FUNCTION yog_price_max_age_asof() RETURNS INTERVAL
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT INTERVAL '1 hour' $$;

COMMENT ON FUNCTION yog_price_max_age_asof() IS
    'How far BEFORE the valued event/bucket a price observation may have been '
    'fetched and still describe it. One bucket width.';

CREATE FUNCTION yog_price_max_age_latest() RETURNS INTERVAL
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT INTERVAL '15 minutes' $$;

COMMENT ON FUNCTION yog_price_max_age_latest() IS
    'How old the most recent price observation may be, relative to now(), and '
    'still count as a current price. 30 ticks of the yog-context price worker.';


-- ── drop the dependents, innermost last ─────────────────────────────────────
-- Only one view-on-view dependency exists among the six: 019 selects from the
-- priced view of 002. The other four are independent.

DROP VIEW meteora_damm_v2_pool_hourly_activity;
DROP VIEW meteora_damm_v2_swap_events_hourly_priced;
DROP VIEW pool_current_tvl;
DROP VIEW meteora_damm_v2_liquidity_events_valued;
DROP VIEW meteora_damm_v2_pool_hourly_flow;
DROP VIEW meteora_damm_v2_pool_hourly_liquidity_flow;


-- ── meteora_damm_v2_swap_events_hourly_priced (002, + staleness) ────────────
-- Unchanged but for the as-of bound on both price LATERALs. See 002 for why
-- `token_metadata` and `pools` are LEFT-joined, why every zero is `NULLIF`-ed,
-- and what `valuation_complete` decides.
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


-- ── meteora_damm_v2_pool_hourly_activity (019, + staleness) ─────────────────
-- Unchanged but for the as-of bound on the five price LATERALs of `liq_v`,
-- `pos_fee_v` and `reward_v`. `swap_v` needs none: it reads the priced view
-- above, which now carries the bound itself.
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
        (COALESCE(h.amount_a_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.amount_b_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_added_usd,
        (COALESCE(h.amount_a_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.amount_b_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_removed_usd
    FROM meteora_damm_v2_liquidity_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket AND fetched_at >= h.bucket - yog_price_max_age_asof() ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
pos_fee_v AS (
    SELECT h.pool_address, h.bucket,
        (COALESCE(h.fee_a_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.fee_b_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_claimed_usd
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


-- ── pool_current_tvl (020, + staleness) ────────────────────────────────────
-- The LATEST-price case, and the one that motivated this migration: the only
-- bound here is against `now()`. `tvl_usd` was already NULL when a token had no
-- known price; it is now equally NULL when the price is older than the policy
-- allows — which is what makes a `yog-context` outage visible instead of
-- publishing yesterday's TVL as today's.
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
      AND fetched_at >= now() - yog_price_max_age_latest()
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint::TEXT
      AND fetched_at >= now() - yog_price_max_age_latest()
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

GRANT SELECT ON pool_current_tvl TO yog_api;
GRANT SELECT ON pool_current_tvl TO yog_signals;


-- ── meteora_damm_v2_liquidity_events_valued (021, + staleness) ─────────────
-- Read by the API's liquidity-event feed, not by a detector — and it follows the
-- same rule: a value the reader cannot trust is not shown as a number. The event
-- row still always appears; only `value_usd` goes NULL and the frontend renders
-- "—", exactly as it already does for an unpriced or unresolved token.
--
-- ⚠️ Column ORDER is load-bearing: `query_as!` maps it by position (041).
CREATE VIEW meteora_damm_v2_liquidity_events_valued AS
SELECT
    le.pool_address,
    le.signature,
    le.timestamp,
    le.liquidity_event_kind,
    le.amount_a,
    le.amount_b,
    le.liquidity_delta,
    le.reserve_a_after,
    le.reserve_b_after,
    le.position,
    le.owner,
    (
        (le.amount_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
      + (le.amount_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
    ) AS value_usd,
    le.slot,
    le.event_index,
    le.transaction_index
FROM meteora_damm_v2_liquidity_events le
LEFT JOIN pools p ON p.pool_address = le.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint AND fetched_at <= le.timestamp
      AND fetched_at >= le.timestamp - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint AND fetched_at <= le.timestamp
      AND fetched_at >= le.timestamp - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

GRANT SELECT ON meteora_damm_v2_liquidity_events_valued TO yog_api;


-- ── meteora_damm_v2_pool_hourly_flow (023, + staleness) ────────────────────
-- Unchanged but for the as-of bound. It still reads the swap cagg directly and
-- still multiplies by the OBSERVED price: switching it to the priced view is
-- ticket 08's decision, not this migration's.
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
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket
      AND fetched_at >= h.bucket - yog_price_max_age_asof()
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true;

GRANT SELECT ON meteora_damm_v2_pool_hourly_flow TO yog_signals;


-- ── meteora_damm_v2_pool_hourly_liquidity_flow (025, + staleness) ──────────
-- Unchanged but for the as-of bound. Its NULL propagation across both token legs
-- is the behaviour ticket 08 wants the other flow view to converge on; nothing
-- about it changes here.
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
