-- The locked liquidity of the TVL reading of $1, at $2: the protocol's figures ($3), every protocol's
-- tokens, and the vesting releases up to $2 + $4. A pool is valued when a side is and its vesting known.
-- ⚠️ MATERIALIZED: the three writes read one evaluation of the function, its cost paid once.
WITH locked AS MATERIALIZED (
    SELECT l.pool_address, l.permanent_liquidity, l.vesting_liquidity,
           l.a_usd_per_liquidity, l.b_usd_per_liquidity,
           COALESCE(l.a_usd_per_liquidity + l.b_usd_per_liquidity,
                    l.a_usd_per_liquidity, l.b_usd_per_liquidity) AS usd_per_liquidity
      FROM yog_damm_v2_pool_locked_liquidity($2) l
),
valued AS (
    SELECT pool_address,
           permanent_liquidity * usd_per_liquidity AS permanent_usd,
           vesting_liquidity * usd_per_liquidity AS vesting_usd,
           (permanent_liquidity + vesting_liquidity) * a_usd_per_liquidity AS a_usd,
           (permanent_liquidity + vesting_liquidity) * b_usd_per_liquidity AS b_usd,
           usd_per_liquidity IS NOT NULL AND vesting_liquidity IS NOT NULL AS is_valued
      FROM locked
),
reading AS (
    UPDATE analytic_protocol_tvl t
       SET locked_usd            = s.locked_usd,
           permanent_usd         = s.permanent_usd,
           vesting_usd           = s.vesting_usd,
           pools_locked          = s.pools,
           pools_locked_unvalued = s.unvalued
      FROM (
        SELECT COALESCE(SUM(permanent_usd + vesting_usd) FILTER (WHERE is_valued), 0) AS locked_usd,
               COALESCE(SUM(permanent_usd) FILTER (WHERE is_valued), 0) AS permanent_usd,
               COALESCE(SUM(vesting_usd) FILTER (WHERE is_valued), 0) AS vesting_usd,
               COUNT(*) AS pools,
               COUNT(*) FILTER (WHERE NOT is_valued) AS unvalued
          FROM valued
      ) s
     WHERE t.protocol = $3 AND t.bucket = $1
    RETURNING 1
),
sides AS (
    SELECT p.protocol, p.token_a_mint::TEXT AS mint, t.value_a_usd AS tvl_usd,
           CASE WHEN v.is_valued THEN v.a_usd END AS locked_usd, v.pool_address IS NOT NULL AS is_locked
      FROM pool_current_tvl t
      JOIN pools p ON p.pool_address = t.pool_address
      LEFT JOIN valued v ON v.pool_address = t.pool_address
     WHERE p.token_a_mint IS NOT NULL
    UNION ALL
    SELECT p.protocol, p.token_b_mint::TEXT, t.value_b_usd,
           CASE WHEN v.is_valued THEN v.b_usd END, v.pool_address IS NOT NULL
      FROM pool_current_tvl t
      JOIN pools p ON p.pool_address = t.pool_address
      LEFT JOIN valued v ON v.pool_address = t.pool_address
     WHERE p.token_b_mint IS NOT NULL
),
tokens AS (
    INSERT INTO analytic_tvl_by_token (protocol, bucket, mint, tvl_usd, locked_usd, pools)
    SELECT protocol, $1, mint, SUM(tvl_usd),
           -- ⚠️ Locked pools none of them valued: unknown, not 0.
           CASE WHEN protocol <> $3 THEN NULL
                WHEN bool_or(is_locked) AND COUNT(locked_usd) = 0 THEN NULL
                ELSE COALESCE(SUM(locked_usd), 0) END,
           COUNT(*)
      FROM sides
     GROUP BY protocol, mint
    HAVING COUNT(tvl_usd) > 0
    RETURNING 1
),
bounds AS (
    SELECT extract(epoch FROM $2::TIMESTAMPTZ)::NUMERIC AS from_s,
           extract(epoch FROM $2::TIMESTAMPTZ + $4::INTERVAL)::NUMERIC AS to_s
),
schedules AS (
    -- ⚠️ Seconds only: a slot-counted schedule's instant would be an estimate.
    SELECT v.pool_address, v.position, v.cliff_point::NUMERIC AS cliff,
           v.period_frequency::NUMERIC AS freq, v.number_of_period AS n,
           v.cliff_unlock_liquidity, v.liquidity_per_period
      FROM meteora_damm_v2_lock_position_events v
      CROSS JOIN bounds b
      LEFT JOIN meteora_damm_v2_pool_properties pp ON pp.pool_address = v.pool_address
     WHERE COALESCE(pp.pool_activation_type,
                    (SELECT i.activation_type FROM meteora_damm_v2_initialize_pool_events i
                      WHERE i.pool_address = v.pool_address LIMIT 1)) = 1
       AND v.cliff_point <= b.to_s
       AND v.cliff_point + v.period_frequency * v.number_of_period > b.from_s
),
releases AS (
    -- The cliff releases its own part; each period after it, one more `liquidity_per_period`.
    SELECT s.pool_address, s.position, s.cliff AS point, s.cliff_unlock_liquidity AS liquidity
      FROM schedules s CROSS JOIN bounds b
     WHERE s.cliff_unlock_liquidity > 0 AND s.cliff > b.from_s
    UNION ALL
    SELECT s.pool_address, s.position, s.cliff + k * s.freq, s.liquidity_per_period
      FROM schedules s CROSS JOIN bounds b
     CROSS JOIN LATERAL generate_series(
         GREATEST(1, floor((b.from_s - s.cliff) / s.freq) + 1)::BIGINT,
         LEAST(s.n::NUMERIC, floor((b.to_s - s.cliff) / s.freq))::BIGINT) AS k
     WHERE s.freq > 0 AND s.liquidity_per_period > 0
),
unlocks AS (
    INSERT INTO analytic_vesting_unlocks
        (protocol, bucket, pool_address, position, unlocks_at, liquidity, amount_usd)
    SELECT $3, $1, r.pool_address, r.position, to_timestamp(r.point), r.liquidity,
           r.liquidity * l.usd_per_liquidity
      FROM releases r
      LEFT JOIN locked l ON l.pool_address = r.pool_address
    RETURNING 1
)
SELECT (SELECT COUNT(*) FROM reading) AS "readings!",
       (SELECT COUNT(*) FROM tokens) AS "tokens!",
       (SELECT COUNT(*) FROM unlocks) AS "unlocks!"
