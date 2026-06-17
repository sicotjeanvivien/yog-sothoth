-- ============================================================================
-- 019 — meteora_damm_v2_pool_hourly_activity (VIEW)
-- ============================================================================
-- A read-time VIEW that encapsulates the per-(pool, hour) USD valuation of the
-- four hourly continuous aggregates (swap, liquidity, claim_position_fee,
-- claim_reward). It exists to move a large, twice-duplicated analytical query
-- OUT of the Rust string literals and into versioned SQL — and to DRY it:
-- before this, the as-of-bucket trade-time valuation (the `LATERAL token_prices`
-- + `POWER(10::NUMERIC, decimals)` joins) was copy-pasted in BOTH
-- `pool_analytics.batch_compute` (24h roll-up) and `pool_analytics.history`
-- (per-bucket series). Both now read this single view; their Rust queries
-- collapse to a trivial SELECT that the sqlx macro still verifies against the
-- view's columns.
--
-- Not parameterized (a VIEW can't take args): it values every pool / every
-- bucket. Callers filter with `WHERE pool_address = … AND bucket > …`; Postgres
-- pushes those predicates down into the underlying CAs, so a single-pool read
-- only touches that pool's recent buckets (no materialization, planner-inlined).
--
-- Valuation mirrors the previous inline logic exactly: raw CA token amounts
-- divided by 10^decimals and priced at the most recent `token_prices` row
-- as-of the bucket (trade-time, not current price). A pool whose mints aren't
-- resolved yet drops out (INNER JOIN on token_metadata) → no row. Reward claims
-- are valued by their own reward mint and summed across mints per bucket.

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
        (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS volume_usd,
        (COALESCE(h.fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_usd,
        (COALESCE(h.protocol_fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.protocol_fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS protocol_fees_usd,
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
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
