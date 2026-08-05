-- ============================================================================
-- 002_swap_implied_price.sql — value a swap bucket by whichever side is priced
-- ============================================================================
-- Fixes the finding of `.project` ticket 02: a bucket whose tokens are not BOTH
-- priced came out NULL *in full*, and `SUM` then skipped it — publishing a
-- sub-total as if it were a total. Measured 3 August 2026: 38,4 % of the hourly
-- buckets containing swaps had `volume_usd = NULL`, and nothing said so.
--
-- ## The mechanism being fixed
--
-- §15's valuation multiplied each side's amount by that side's observed price:
--
--     (volume_in_a / 10^dec_a) * pa.price_usd
--   + (volume_in_b / 10^dec_b) * pb.price_usd
--
-- NULL is contagious in SQL arithmetic, so ONE missing price annihilated the
-- whole expression — including the leg that was perfectly valuable. The loss
-- was not the unknown part, it was the entire row.
--
-- ## The fix, in one notion: the *effective* price
--
-- A swap is an exchange: its two legs are the same trade, so when one token is
-- priced the bucket's USD notional is known. This migration therefore adds the
-- effective price of a token in a bucket:
--
--     its observed price, else the rate this bucket's own swaps traded at,
--     anchored on the other side's observed price.
--
-- That rate is MEASURED, not extrapolated — it comes from trades that actually
-- happened inside the hour, and it is anchored on the hard asset (SOL, USDC),
-- which is also the more robust of the two: a wash trade at an absurd price
-- inflates the memecoin leg, not the SOL leg. When NEITHER side is priced, both
-- effective prices stay NULL and the bucket stays NULL — "we don't know" is
-- preserved, no fallback onto a later price is introduced.
--
-- ## The limit that matters: the implied rate is net of the fee
--
-- One of the two legs is net of the trading fee and the other is not — which
-- leg depends on the pool's `collect_fee_mode`: mode 0 (BothToken) charges it on
-- the OUT token, modes 1 and 2 always on token B, in or out (see
-- `compute_fee_token_is_a` in core's swap translator). Either way the fee side
-- is short by `f` relative to the other, so the relation below holds in every
-- mode. Writing X for the hour's a→b input value and Y for its b→a input value,
-- the implied rate relates to the true one by
--
--     implied / true = (X(1-f) + Y) / (X + Y(1-f))
--
-- which is exactly 1 when X = Y — and departs from it by the full fee rate `f`
-- when the hour traded in ONE direction only.
--
-- ⚠️ **That symmetric case is not the one we are in.** Measured on 5 August
-- 2026: of the 36 buckets that actually used an implied rate, **35 were
-- one-directional**. Which stands to reason — a bucket needing an implied rate
-- is a bucket on a thin, unlisted token, and thin tokens trade one way at a
-- time. The cancellation is real algebra and a rare event; do not read it as a
-- mitigation.
--
-- So, concretely, for a one-directional bucket:
--
--   * `implied_price = true_price × (1 - f)`, hence `volume_usd` is the value
--     of the **output** leg — the volume NET of fee;
--   * a bucket whose two prices were observed values the **input** leg — the
--     volume GROSS of fee.
--
-- **Two conventions therefore coexist in one column**, selected by whether a
-- price happened to be observed. At 25 bps the difference is invisible. It is
-- not invisible on a launch pool with a fee scheduler — and those are precisely
-- the pools whose token is not listed yet, so the adverse correlation is the
-- same one this whole ticket is about.
--
-- The magnitude cannot be stated honestly today: it is bounded by the pool's
-- REAL fee rate, and `pools.fee_bps` is frozen on the scheduler cliff (ticket
-- 07 measured it wrong by ×5 and ×49). Values from 4 to 9900 bps are recorded,
-- and no one can currently tell which are real.
--
-- This is still a large net gain — a NULL carries no information at all, an
-- approximation carries most of it. But it IS an approximation with an unknown
-- bound rather than a sub-percent one, and that had better be written where the
-- number is defined rather than assumed to be common knowledge.
--
-- `price_a_implied` / `price_b_implied` stop at this view on purpose: surfacing
-- "this figure used a derived rate" in the API and the dashboard is a product
-- decision, not a persistence one, and it is not made here. Until it is, the
-- honest place for this caveat is the schema and the READMEs — which is where
-- it is.
--
-- Second, smaller limit: the rate is the hour's volume-weighted average,
-- applied to that same hour's volume and fees — which is what it is the right
-- average for.
--
-- ## Scope: swaps only
--
-- §15's `liq_v` / `pos_fee_v` / `reward_v` are NOT touched. A liquidity add has
-- no counter-leg anchoring it, so applying a trade rate to it would be an
-- extrapolation rather than a measurement. The rule is: a rate is only used on
-- the flow that produced it. Those CTEs keep returning NULL on a missing price
-- (see ticket 08 for the convention question they raise).
--
-- ## Why the cagg is dropped and rebuilt rather than doubled
--
-- A continuous aggregate cannot be ALTERed, and the two new raw sums have to
-- live in it. A second, additive cagg carrying only the totals would have
-- avoided the drop — at the price of two overlapping aggregates over the same
-- hypertable, two refresh jobs, and a duplicated truth to keep in sync forever:
-- exactly the accretion the 5 August squash had just removed.
--
-- The drop costs nothing *today*: no cagg has ever materialized a bucket (the
-- TimescaleDB job scheduler has been off since 16 June — ticket 03), and
-- `materialized_only = false` keeps reads correct through real-time
-- aggregation. Rebuilt `WITH NO DATA` + policy, exactly as §13 leaves it, so
-- this migration does not change ticket 03's situation in either direction.
--
-- §15's two other swap-cagg readers are dropped only because they depend on it:
-- `meteora_damm_v2_pool_hourly_activity` is rebuilt USING the new price view,
-- `meteora_damm_v2_pool_hourly_flow` is restored verbatim (its own missing-price
-- asymmetry is ticket 08's decision, not this migration's).
-- ============================================================================


-- ── drop the dependents, then the aggregate ─────────────────────────────────
DROP VIEW meteora_damm_v2_pool_hourly_activity;
DROP VIEW meteora_damm_v2_pool_hourly_flow;
DROP MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly;


-- ── swap volume + realized fees, rebuilt  [was §13, 010/014/017] ────────────
-- Unchanged from the baseline except for the last two columns.
--
-- `volume_in_a` / `volume_in_b` stay direction-filtered: the volume convention
-- counts only the INPUT side of each swap (a_to_b → amount_a, b_to_a →
-- amount_b), so a swap is counted once.
--
-- `traded_a` / `traded_b` are new and are NOT filtered: they are every token A
-- and every token B that moved through the pool during the hour, whichever
-- direction it moved in. They are the two faces of the same set of exchanges,
-- so their USD values are equal up to the fee — which is what makes one
-- derivable from the other in the price view below. They are raw amounts, in
-- keeping with §13's rule that a cagg cannot join token_prices and therefore
-- stores no USD.
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


-- ── meteora_damm_v2_swap_events_hourly_priced (002) ─────────────────────────
-- The swap cagg, carrying everything needed to value it: the token decimals,
-- the effective price of each side, and whether that price had to be implied.
--
-- ## Why it carries the cagg's own columns rather than just the prices
--
-- A first version exposed only `(pool_address, bucket, eff_price_*)`, and the
-- activity view joined it *alongside* the cagg. Inlining does not dedupe that:
-- the plan then scanned the swap hypertable TWICE (`_hyper_4_1_chunk` and
-- `_hyper_4_1_chunk_1` in `EXPLAIN`), and recomputed the pools/token_metadata
-- join twice with it — on the read path behind `/api/stats` and every pool
-- list. Passing the cagg's columns through means the activity view selects
-- from this view ALONE, and the hypertable is scanned once.
--
-- So: a plain VIEW is inlined and costs nothing *of its own*, but a view that
-- re-reads a table its caller also reads is not free. That is the trap here.
--
-- ## Why token_metadata is LEFT-joined, unlike everywhere else in §15
--
-- §15's views INNER-join `token_metadata`, so a pool whose mints are not
-- resolved yet produces NO ROW — the first of the three ways this codebase says
-- "we don't know" (the row disappears). That is fine for a value, and wrong for
-- a *coverage denominator*: the buckets that vanish are exactly the ones we
-- failed to value, so counting only the surviving rows would report a pool as
-- 100 % covered while its volume was silently missing — the very defect this
-- migration exists to remove, moved one join up.
--
-- LEFT-joining keeps the bucket with NULL decimals, hence a NULL valuation, so
-- it lands in the denominator and not in the numerator.
--
-- `pools` is LEFT-joined for the same reason, and it is NOT redundant. The
-- persistor does call `discover_pool` before inserting a swap — but that call
-- is skip-and-log (`event_persistor.rs`: the error is warned and the insert
-- proceeds anyway), so a failed upsert leaves a swap whose pool has no row. An
-- INNER join would make that bucket vanish from both sides of the coverage
-- ratio: the exact defect this migration removes, one join further up. Zero
-- occurrences measured today; the guarantee is probabilistic, not structural,
-- and a coverage denominator must not rest on one.
--
-- ## The rule itself
--
-- `implied_a` reads the OTHER side's observed price (`pb`): a rate cannot be
-- derived from an unknown anchor, so when `pb` is NULL `implied_a` is NULL too,
-- and a bucket with neither side priced yields two NULL effective prices — the
-- "we don't know" case, unchanged.
--
-- `price_a_implied` / `price_b_implied` say when the fallback was actually used
-- — which makes it auditable rather than invisible:
--
--     SELECT count(*) FILTER (WHERE price_a_implied OR price_b_implied), count(*)
--     FROM meteora_damm_v2_swap_events_hourly_priced;
--
-- **Every zero in the ratio is `NULLIF`-ed — the amounts AND the price.** A zero
-- divisor is the obvious case (no division by zero). The treacherous one is a
-- zero *numerator*: it yields a clean `0`, hence `eff_price = 0`,
-- `price_*_implied = true`, and a bucket valued at `volume_usd = 0` — counted as
-- COVERED while the figure is fabricated. Coverage would read 1/1 on a lie,
-- which is the exact defect this migration exists to remove.
--
-- Two ways in, and both are shut:
--
--   * `traded_x = 0` — a bucket whose swaps moved none of a token has nothing
--     to anchor that token's rate on;
--   * `price_usd = 0` — and this one is not hypothetical. `token_prices.price_usd`
--     is `NUMERIC(38, 18)`, so any price below 5e-19 **rounds to exactly zero on
--     insert** (measured: `0.00000000000000000000123::NUMERIC(38,18) = 0`), and
--     the column carries no `CHECK (> 0)`. Very-high-supply memecoins live in
--     precisely that range — which is to say, the population this migration
--     exists to rescue. A zero price is not a price.
--
-- **What this view can and cannot recover.** It fixes the missing *price*. It
-- does not fix missing *metadata*: `dec_a` / `dec_b` come from `token_metadata`,
-- and a NULL decimal makes `POWER(10, NULL)` NULL, which annihilates that leg
-- and the bucket with it — even when the other side's price is observed. That
-- is correct (an amount cannot be converted without its scale) but it means the
-- real bound on valuation is the *metadata*, not the price. The reverse cannot
-- happen: `PriceWorker` prices the known mints, so a price never exists without
-- its metadata row.
--
-- No explicit GRANT, like §15's other derived views: `setup_roles.sql` sets
-- `ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate … GRANT SELECT` for all four
-- runtime roles, so they can already read it. The explicit GRANTs in this file
-- are the ones the privilege matrix asserts (`tests/privileges.rs` compares
-- explicit grants only); nothing here needs to be in that matrix.
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
    COALESCE(pa.price_usd, i.implied_a) AS eff_price_a,
    COALESCE(pb.price_usd, i.implied_b) AS eff_price_b,
    -- "implied" means the implied rate was actually USED — false both when the
    -- observed price was there and when nothing could be derived at all.
    (pa.price_usd IS NULL AND i.implied_a IS NOT NULL) AS price_a_implied,
    (pb.price_usd IS NULL AND i.implied_b IS NOT NULL) AS price_b_implied
FROM meteora_damm_v2_swap_events_hourly h
LEFT JOIN pools p ON p.pool_address = h.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true
CROSS JOIN LATERAL (
    SELECT
        ((NULLIF(h.traded_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals))
            * NULLIF(pb.price_usd, 0))
            / NULLIF(h.traded_a::NUMERIC / POWER(10::NUMERIC, tma.decimals), 0)
            AS implied_a,
        ((NULLIF(h.traded_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals))
            * NULLIF(pa.price_usd, 0))
            / NULLIF(h.traded_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals), 0)
            AS implied_b
) i;


-- ── meteora_damm_v2_pool_hourly_activity (019, rebuilt) ─────────────────────
-- Same contract and same output columns as the baseline. The change is confined
-- to `swap_v`: it now selects from the priced view above and from nothing else
-- — no `pool_tokens`, no `token_prices` LATERALs — and the three USD
-- expressions multiply by the EFFECTIVE price instead of the observed one.
-- Volume, fees and protocol fees move together: they share one valuation, so a
-- bucket that used to lose all three now keeps all three.
--
-- Reading a single source is also what keeps the swap hypertable scanned once;
-- see the note on the priced view.
--
-- `liq_v`, `pos_fee_v` and `reward_v` are byte-for-byte the baseline's: see the
-- scope note in this file's header. They keep the INNER `pool_tokens` join, so
-- for THEM an unresolved pool still produces no row. The asymmetry is
-- deliberate and narrow: only the swap side carries a coverage claim to the
-- API, and only it needs its unvaluable buckets to remain visible.
--
-- Reward claims are valued by their own reward mint and summed across mints per
-- bucket.
CREATE VIEW meteora_damm_v2_pool_hourly_activity AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
-- ⚠️ Each leg is wrapped in `CASE WHEN <amount> = 0 THEN 0`, and that is not
-- noise. `0 * NULL` is NULL in SQL, and so is `0 / POWER(10, NULL)` — so a leg
-- carrying NO tokens was annihilating buckets that were fully computable from
-- the other side. Concretely: a pool whose token B has no `token_metadata` row
-- yet (the metadata worker upserts mint by mint and absorbs per-row failures),
-- trading a→b only, produced NULL on a bucket worth exactly `volume_in_a ×
-- price_a`. Nothing about the B side needed converting — there was no B.
--
-- The distinction the CASE draws is the whole point, and the two branches are
-- NOT interchangeable:
--   * amount = 0            → contribute 0. Nothing moved; its value is zero in
--                             any currency, known without any price.
--   * amount > 0, price NULL → stay NULL. Something moved and we cannot value
--                             it; the bucket must not pass for complete.
-- A blanket `COALESCE(…, 0)` would collapse the second case into the first,
-- which is the "unknown becomes zero" sin catalogued in the persistence README.
swap_v AS (
    SELECT h.pool_address, h.bucket,
        CASE WHEN COALESCE(h.volume_in_a, 0) = 0 THEN 0
             ELSE (h.volume_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
      + CASE WHEN COALESCE(h.volume_in_b, 0) = 0 THEN 0
             ELSE (h.volume_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
        AS volume_usd,
        CASE WHEN COALESCE(h.fee_in_a, 0) = 0 THEN 0
             ELSE (h.fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
      + CASE WHEN COALESCE(h.fee_in_b, 0) = 0 THEN 0
             ELSE (h.fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
        AS fees_usd,
        CASE WHEN COALESCE(h.protocol_fee_in_a, 0) = 0 THEN 0
             ELSE (h.protocol_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
      + CASE WHEN COALESCE(h.protocol_fee_in_b, 0) = 0 THEN 0
             ELSE (h.protocol_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
        AS protocol_fees_usd,
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
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
pos_fee_v AS (
    SELECT h.pool_address, h.bucket,
        (COALESCE(h.fee_a_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.fee_b_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_claimed_usd
    FROM meteora_damm_v2_claim_position_fee_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
reward_v AS (
    SELECT h.pool_address, h.bucket,
        SUM((COALESCE(h.total_reward, 0)::NUMERIC / POWER(10::NUMERIC, tmr.decimals)) * pr.price_usd) AS rewards_claimed_usd
    FROM meteora_damm_v2_claim_reward_events_hourly h
    JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = h.mint_reward::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pr ON true
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


-- ── meteora_damm_v2_pool_hourly_flow (023, restored verbatim) ───────────────
-- Byte-for-byte the baseline definition. It is dropped and recreated here only
-- because it reads the swap cagg; nothing about it changes.
--
-- It could now read `eff_price_*` in one line, which is precisely the fix ticket
-- 08 calls for — its `COALESCE(…, 0)` downstream turns a missing price into a
-- Critical `flow_imbalance` at exactly -1. That changes signal-engine behaviour
-- and belongs to that ticket's decision, not to this one.
--
-- Per-direction USD swap volume per (pool, hour), feeding the Signal Engine's
-- flow-imbalance detector. Same valuation as the activity view's `swap_v` CTE,
-- but the two trade directions are kept SEPARATE — 019 sums them into a single
-- `volume_usd`, and a flow imbalance needs each side on its own.
--
-- A separate, single-purpose view rather than more columns on 019: 019's
-- contract is the four-aggregate activity roll-up; this stays focused.
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
