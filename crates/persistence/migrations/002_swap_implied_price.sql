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
-- ## Two known limits, deliberately accepted
--
--   1. `amount_in` includes the fee and `amount_out` does not, so the implied
--      rate is biased by at most the pool's fee rate (~0,25 %), and the bias
--      partially cancels when the hour's flow is balanced. Sub-percent, against
--      a 100 % loss today.
--   2. The rate is the hour's volume-weighted average, applied to that same
--      hour's volume and fees — which is what it is the right average for.
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


-- ── meteora_damm_v2_pool_hourly_price (002) ─────────────────────────────────
-- The effective price of each token, per (pool, hour) that had swaps.
--
-- One row per swap bucket, carrying the price to value that bucket with and
-- whether it had to be implied. Split out of the activity view rather than
-- inlined so that the rule has one definition, and so that "which buckets did
-- we have to imply a price for?" is a query rather than an audit:
--
--     SELECT count(*) FILTER (WHERE price_a_implied OR price_b_implied), count(*)
--     FROM meteora_damm_v2_pool_hourly_price;
--
-- A plain VIEW, so no performance effect of its own — Postgres inlines it.
--
-- `implied_a` reads the OTHER side's observed price (`pb`): a rate cannot be
-- derived from an unknown anchor, so when `pb` is NULL `implied_a` is NULL too,
-- and a bucket with neither side priced yields two NULL effective prices — the
-- "we don't know" case, unchanged.
--
-- `NULLIF(…, 0)` guards the division: a bucket whose swaps moved no token A at
-- all (theoretically possible, e.g. a zero-amount swap) yields NULL rather than
-- a division-by-zero error.
--
-- Definer's-rights view like every other one in §15, and deliberately WITHOUT a
-- GRANT: its only reader is the activity view below, which is owned by the same
-- role and therefore reads it on its own rights. Adding a grant would widen the
-- privilege surface for nobody. (Ticket 08 will want `yog_signals` on it — that
-- grant belongs to the migration that makes `flow_imbalance` read it.)
CREATE VIEW meteora_damm_v2_pool_hourly_price AS
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
    COALESCE(pa.price_usd, i.implied_a) AS eff_price_a,
    COALESCE(pb.price_usd, i.implied_b) AS eff_price_b,
    -- "implied" means the implied rate was actually USED — false both when the
    -- observed price was there and when nothing could be derived at all.
    (pa.price_usd IS NULL AND i.implied_a IS NOT NULL) AS price_a_implied,
    (pb.price_usd IS NULL AND i.implied_b IS NOT NULL) AS price_b_implied
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
) pb ON true
CROSS JOIN LATERAL (
    SELECT
        ((h.traded_b::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd)
            / NULLIF(h.traded_a::NUMERIC / POWER(10::NUMERIC, pt.dec_a), 0)
            AS implied_a,
        ((h.traded_a::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd)
            / NULLIF(h.traded_b::NUMERIC / POWER(10::NUMERIC, pt.dec_b), 0)
            AS implied_b
) i;


-- ── meteora_damm_v2_pool_hourly_activity (019, rebuilt) ─────────────────────
-- Same contract and same output columns as the baseline. The only change is in
-- `swap_v`: its two `token_prices` LATERAL joins are replaced by a join on the
-- price view above, and the three USD expressions multiply by the EFFECTIVE
-- price instead of the observed one. Volume, fees and protocol fees all move
-- together — they share the join, so a bucket that used to lose all three now
-- keeps all three.
--
-- `liq_v`, `pos_fee_v` and `reward_v` are byte-for-byte the baseline's: see the
-- scope note in this file's header.
--
-- A pool whose mints are not resolved yet drops out (INNER JOIN on
-- token_metadata) → no row. Reward claims are valued by their own reward mint
-- and summed across mints per bucket.
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
        (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * ep.eff_price_a
      + (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * ep.eff_price_b AS volume_usd,
        (COALESCE(h.fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * ep.eff_price_a
      + (COALESCE(h.fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * ep.eff_price_b AS fees_usd,
        (COALESCE(h.protocol_fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * ep.eff_price_a
      + (COALESCE(h.protocol_fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * ep.eff_price_b AS protocol_fees_usd,
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    JOIN meteora_damm_v2_pool_hourly_price ep
        ON ep.pool_address = h.pool_address AND ep.bucket = h.bucket
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
