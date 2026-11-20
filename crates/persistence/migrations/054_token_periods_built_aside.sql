-- 054_token_periods_built_aside.sql — the tokens of every window, built in parallel in a work table, and only what
-- differs written to analytic_token_periods: about 2 % of its rows change from one freeze to the next. `bucket` leaves
-- the rows, which it would all change: the hour of the ranking is the last frozen one, frozen in the same transaction.
-- SECURITY DEFINER, so yog_analytic needs EXECUTE alone; search_path pinned with pg_temp last, as in 013.

ALTER TABLE analytic_token_periods DROP COLUMN bucket;

CREATE FUNCTION yog_replace_token_periods(through TIMESTAMPTZ, periods TEXT[], hours INT[])
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    -- ⚠️ A variable, not a subquery: known when the chunks are chosen, the older ones are left out.
    oldest TIMESTAMPTZ := through - make_interval(
        hours => (SELECT max(h) FROM UNNEST(yog_replace_token_periods.hours) AS h));
BEGIN
    CREATE UNLOGGED TABLE analytic_token_periods_build AS
    WITH by_pool AS (
        SELECT w.period, f.protocol, f.pool_address,
               SUM(f.volume_in_a_usd + f.volume_in_b_usd) FILTER (WHERE f.valued) AS volume_usd,
               COUNT(*) AS pool_hours, COUNT(*) FILTER (WHERE f.valued) AS pool_hours_valued
          FROM UNNEST(yog_replace_token_periods.periods, yog_replace_token_periods.hours)
                   AS w (period, hours)
          JOIN analytic_pool_hourly f
            ON f.bucket > through - make_interval(hours => w.hours)
           AND f.bucket <= through
         -- ⚠️ The longest window again, alone: the per-window bound cannot leave a chunk out.
         WHERE f.bucket > oldest
         GROUP BY w.period, f.protocol, f.pool_address
    )
    -- ⚠️ Rounded: the page cursor, a Decimal, must hold the volume exactly.
    SELECT b.period, m.mint, ROUND(COALESCE(SUM(b.volume_usd), 0), 6) AS volume_usd,
           COUNT(*) AS pools, SUM(b.pool_hours) AS pool_hours,
           SUM(b.pool_hours_valued) AS pool_hours_valued
      FROM by_pool b
      -- ⚠️ The mints joined to the pools once summed, not to each pool-hour.
      JOIN pools p ON p.pool_address = b.pool_address AND p.protocol = b.protocol
     CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY b.period, m.mint;

    -- ⚠️ DELETE, not TRUNCATE: its lock would wait behind pg_dump and block every reader.
    DELETE FROM analytic_token_periods t
     WHERE NOT EXISTS (SELECT 1 FROM analytic_token_periods_build b
                        WHERE b.period = t.period AND b.mint = t.mint);
    -- ⚠️ Filtered before the ON CONFLICT: one whose WHERE is false still locks every row.
    INSERT INTO analytic_token_periods
        (period, mint, volume_usd, pools, pool_hours, pool_hours_valued)
    SELECT b.period, b.mint, b.volume_usd, b.pools, b.pool_hours, b.pool_hours_valued
      FROM analytic_token_periods_build b
      LEFT JOIN analytic_token_periods t ON t.period = b.period AND t.mint = b.mint
     WHERE t.mint IS NULL
        OR (t.volume_usd, t.pools, t.pool_hours, t.pool_hours_valued)
           IS DISTINCT FROM (b.volume_usd, b.pools, b.pool_hours, b.pool_hours_valued)
    ON CONFLICT (period, mint) DO UPDATE SET
        volume_usd        = EXCLUDED.volume_usd,
        pools             = EXCLUDED.pools,
        pool_hours        = EXCLUDED.pool_hours,
        pool_hours_valued = EXCLUDED.pool_hours_valued;
    DROP TABLE analytic_token_periods_build;
END;
$$;

REVOKE ALL ON FUNCTION yog_replace_token_periods(TIMESTAMPTZ, TEXT[], INT[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION yog_replace_token_periods(TIMESTAMPTZ, TEXT[], INT[]) TO yog_analytic;
REVOKE INSERT, DELETE ON analytic_token_periods FROM yog_analytic;
