-- 057_locked_liquidity.sql — the DAMM v2 liquidity locked at each TVL reading, permanent and vesting,
-- valued at the TVL's prices; the TVL by token at the last reading; the vesting releases of the next
-- 30 days. And the pool's own unit of time, read from its account for every pool.

-- ── The pool's unit, whatever its fee shape ──────────────────────────────────────────────────
ALTER TABLE meteora_damm_v2_pool_properties ADD COLUMN pool_activation_type SMALLINT;
COMMENT ON COLUMN meteora_damm_v2_pool_properties.pool_activation_type IS
    'Unit of every point of the pool, a vesting schedule''s included: 0 slot, 1 timestamp.';

-- ⚠️ The flag, not the unresolved predicate, as in 056: a NULL there would requeue a short account forever.
UPDATE pools SET needs_refresh = true WHERE protocol = 'meteora_damm_v2';

-- ── What L is worth ──────────────────────────────────────────────────────────────────────────
-- The amounts of L in a concentrated pool, unrounded.
-- ⚠️ A compounding pool has no range: its amounts are a share of the reserves, not this.
CREATE FUNCTION yog_damm_v2_liquidity_amounts(
    l NUMERIC, sqrt_price NUMERIC, sqrt_min NUMERIC, sqrt_max NUMERIC,
    OUT amount_a NUMERIC, OUT amount_b NUMERIC)
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT l * (sqrt_max - p) / NULLIF(p * sqrt_max, 0),
           l * (p - sqrt_min) / 340282366920938463463374607431768211456
      FROM (SELECT LEAST(GREATEST(sqrt_price, sqrt_min), sqrt_max) AS p) AS clamped
$$;

-- Every DAMM v2 pool holding locked liquidity at `at`, in L, and what one L is worth on each side.
-- ⚠️ The permanent total is the greater of the account's and the last lock's: a read just before a
-- lock can outlive its refresh flag. A vesting of unknown unit leaves its pool's vesting NULL.
CREATE FUNCTION yog_damm_v2_pool_locked_liquidity(at TIMESTAMPTZ)
RETURNS TABLE (pool_address TEXT, permanent_liquidity NUMERIC, vesting_liquidity NUMERIC,
               a_usd_per_liquidity NUMERIC, b_usd_per_liquidity NUMERIC)
LANGUAGE sql STABLE AS $$
    WITH point AS (
        SELECT extract(epoch FROM at)::NUMERIC AS secs,
               -- ⚠️ NULLIF: the row is seeded at slot 0, before the indexer's first report.
               NULLIF((SELECT slot FROM network_status WHERE id = 1), 0)::NUMERIC AS slot
    ),
    last_permanent AS (
        -- ⚠️ MAX, not the last event: a pool's total never falls, and two locks of one block have no order.
        SELECT e.pool_address, MAX(e.total_permanent_locked_liquidity) AS l
          FROM meteora_damm_v2_permanent_lock_position_events e
         GROUP BY e.pool_address
    ),
    permanent AS (
        SELECT pp.pool_address, pp.permanent_lock_liquidity AS l
          FROM meteora_damm_v2_pool_properties pp
         WHERE pp.permanent_lock_liquidity > 0
        UNION ALL
        SELECT pool_address, l FROM last_permanent
    ),
    schedules AS (
        SELECT v.pool_address,
               v.cliff_unlock_liquidity + v.liquidity_per_period * v.number_of_period AS total,
               v.cliff_point, v.period_frequency, v.cliff_unlock_liquidity, v.liquidity_per_period,
               v.number_of_period,
               -- ⚠️ COALESCE, not a join: the creation event is looked up only when the account gave no unit.
               CASE COALESCE(pp.pool_activation_type,
                             (SELECT ip.activation_type FROM meteora_damm_v2_initialize_pool_events ip
                               WHERE ip.pool_address = v.pool_address LIMIT 1))
                   WHEN 1 THEN point.secs WHEN 0 THEN point.slot END AS now_point
          FROM meteora_damm_v2_lock_position_events v
          CROSS JOIN point
          LEFT JOIN meteora_damm_v2_pool_properties pp ON pp.pool_address = v.pool_address
    ),
    vesting AS (
        -- ⚠️ CASE, not SUM alone: SUM skips the NULL of a schedule whose unit is unknown.
        SELECT s.pool_address,
               CASE WHEN bool_or(s.now_point IS NULL) THEN NULL
                    ELSE SUM(s.total - CASE
                        WHEN s.now_point < s.cliff_point THEN 0
                        WHEN s.period_frequency = 0 OR s.liquidity_per_period = 0 THEN s.cliff_unlock_liquidity
                        ELSE s.cliff_unlock_liquidity + s.liquidity_per_period * LEAST(
                            floor((s.now_point - s.cliff_point) / s.period_frequency), s.number_of_period)
                    END) END AS l
          FROM schedules s
         WHERE s.now_point IS NULL
            OR s.now_point < s.cliff_point + s.period_frequency * s.number_of_period
         GROUP BY s.pool_address
    ),
    locked AS (
        SELECT pool_address, MAX(l) AS permanent FROM permanent GROUP BY pool_address
    )
    SELECT p.pool_address,
           COALESCE(lk.permanent, 0),
           CASE WHEN vs.pool_address IS NULL THEN 0 ELSE vs.l END,
           yog_amount_usd(amt.amount_a, tma.decimals, tpa.price_usd),
           yog_amount_usd(amt.amount_b, tmb.decimals, tpb.price_usd)
      FROM (SELECT pool_address FROM locked UNION SELECT pool_address FROM vesting) p
      LEFT JOIN locked lk ON lk.pool_address = p.pool_address
      LEFT JOIN vesting vs ON vs.pool_address = p.pool_address
      JOIN pools po ON po.pool_address = p.pool_address
      LEFT JOIN meteora_damm_v2_pool_properties pp ON pp.pool_address = p.pool_address
      LEFT JOIN pool_current_state pcs ON pcs.pool_address = p.pool_address
      LEFT JOIN token_metadata tma ON tma.mint = po.token_a_mint::TEXT
      LEFT JOIN token_metadata tmb ON tmb.mint = po.token_b_mint::TEXT
      LEFT JOIN token_stock_price tpa ON tpa.mint = po.token_a_mint::TEXT
      LEFT JOIN token_stock_price tpb ON tpb.mint = po.token_b_mint::TEXT
      LEFT JOIN LATERAL yog_damm_v2_liquidity_amounts(
          1, COALESCE(pcs.last_sqrt_price, pp.sqrt_price), pp.sqrt_min_price, pp.sqrt_max_price) amt
          ON pp.collect_fee_mode IN (0, 1)
$$;

-- ── The reading's figures ────────────────────────────────────────────────────────────────────
-- NULL on the readings written before, and on a protocol without locks.
ALTER TABLE analytic_protocol_tvl
    ADD COLUMN locked_usd            NUMERIC,
    ADD COLUMN permanent_usd         NUMERIC,
    ADD COLUMN vesting_usd           NUMERIC,
    ADD COLUMN pools_locked          BIGINT,
    ADD COLUMN pools_locked_unvalued BIGINT;

-- The last reading only, every token with a valued side; `locked_usd` NULL for a protocol without locks.
CREATE TABLE analytic_tvl_by_token (
    protocol   TEXT        NOT NULL,
    bucket     TIMESTAMPTZ NOT NULL,
    mint       TEXT        NOT NULL,
    tvl_usd    NUMERIC     NOT NULL,
    locked_usd NUMERIC,
    pools      BIGINT      NOT NULL,
    PRIMARY KEY (protocol, mint)
);
CREATE INDEX analytic_tvl_by_token_by_tvl ON analytic_tvl_by_token (protocol, tvl_usd DESC, mint);
CREATE INDEX analytic_tvl_by_token_by_locked
    ON analytic_tvl_by_token (protocol, locked_usd DESC NULLS LAST, mint);

-- Every release of the next 30 days, as of the last reading. A slot-counted vesting has no instant.
CREATE TABLE analytic_vesting_unlocks (
    protocol     TEXT           NOT NULL,
    bucket       TIMESTAMPTZ    NOT NULL,
    pool_address TEXT           NOT NULL,
    position     TEXT           NOT NULL,
    unlocks_at   TIMESTAMPTZ    NOT NULL,
    liquidity    NUMERIC(39, 0) NOT NULL,
    amount_usd   NUMERIC
);
CREATE INDEX analytic_vesting_unlocks_by_instant ON analytic_vesting_unlocks (protocol, unlocks_at);
CREATE INDEX analytic_vesting_unlocks_by_amount
    ON analytic_vesting_unlocks (protocol, amount_usd DESC NULLS LAST);

GRANT SELECT, INSERT, DELETE ON analytic_tvl_by_token, analytic_vesting_unlocks TO yog_analytic;
GRANT SELECT ON analytic_tvl_by_token, analytic_vesting_unlocks TO yog_api;
-- ⚠️ The function runs as its caller: what `pool_current_tvl` reads as its owner, it reads as yog_analytic.
GRANT SELECT ON meteora_damm_v2_pool_properties, network_status, meteora_damm_v2_lock_position_events,
                meteora_damm_v2_permanent_lock_position_events, token_stock_price, pool_current_state
             TO yog_analytic;
