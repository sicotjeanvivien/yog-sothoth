-- The bursts of the pools marked since `$2` less the longer of the gap and an hour: no burst
-- settles later than that. An extinct one waits for its last hour, written whole, to settle.
-- ⚠️ Every read of the tranches goes pool by pool through the key: joined to the bursts,
-- whose count the planner cannot guess, they are read whole.
WITH recent AS (
    SELECT DISTINCT pool_address
      FROM analytic_pool_5min
     WHERE protocol = $1 AND rotation >= $4
       AND bucket >= $2::TIMESTAMPTZ - GREATEST($5::INTERVAL, INTERVAL '1 hour')
       AND bucket <= $3
),
marked AS (
    SELECT r.pool_address, t.bucket,
           CASE WHEN t.bucket - lag(t.bucket) OVER w <= $5::INTERVAL THEN 0 ELSE 1 END AS opens
      FROM recent r
     CROSS JOIN LATERAL (
         SELECT bucket FROM analytic_pool_5min
          WHERE pool_address = r.pool_address AND rotation >= $4 AND bucket <= $3
     ) t
    WINDOW w AS (PARTITION BY r.pool_address ORDER BY t.bucket)
),
drawn AS (
    SELECT pool_address, MIN(bucket) AS started_at, MAX(bucket) AS last
      FROM (SELECT pool_address, bucket,
                   SUM(opens) OVER (PARTITION BY pool_address ORDER BY bucket) AS island
              FROM marked) m
     GROUP BY pool_address, island
    HAVING MAX(bucket) >= $2::TIMESTAMPTZ - GREATEST($5::INTERVAL, INTERVAL '1 hour')
),
-- ⚠️ A settled burst is known by its end: the retention can drop its first tranches, and its
-- start read again would write it a second time.
islands AS (
    SELECT i.*
      FROM drawn i
     WHERE NOT EXISTS (
         SELECT 1 FROM analytic_bursts b
          WHERE b.pool_address = i.pool_address AND b.ended_at = i.last + $6::INTERVAL
            AND b.settled
     )
),
-- ⚠️ As the hourly facts: a pool-hour counts only valued whole, or the part outside
-- the bursts could fall below zero.
pool_hours AS (
    SELECT i.pool_address, i.started_at, x.hour, x.whole
      FROM islands i
     CROSS JOIN LATERAL (
         SELECT time_bucket('1 hour', bucket) AS hour, bool_and(valued) AS whole
           FROM analytic_pool_5min
          WHERE pool_address = i.pool_address
            AND bucket >= time_bucket('1 hour', i.started_at)
            AND bucket < time_bucket('1 hour', i.last) + INTERVAL '1 hour'
          GROUP BY time_bucket('1 hour', bucket)
     ) x
),
tranches AS (
    SELECT i.pool_address, i.started_at, t.bucket,
           CASE WHEN h.whole THEN t.volume_in_a_usd + t.volume_in_b_usd ELSE 0 END AS volume
      FROM islands i
     CROSS JOIN LATERAL (
         SELECT bucket, volume_in_a_usd, volume_in_b_usd
           FROM analytic_pool_5min
          WHERE pool_address = i.pool_address AND bucket >= i.started_at AND bucket <= i.last
     ) t
      JOIN pool_hours h
        ON h.pool_address = i.pool_address AND h.started_at = i.started_at
       AND h.hour = time_bucket('1 hour', t.bucket)
),
written AS (
    INSERT INTO analytic_bursts (
        pool_address, started_at, protocol, ended_at, volume_usd, first_hour_volume_usd,
        extinct, settled
    )
    SELECT i.pool_address, i.started_at, $1, i.last + $6::INTERVAL,
           SUM(t.volume),
           COALESCE(SUM(t.volume) FILTER (WHERE t.bucket < i.started_at + INTERVAL '1 hour'), 0),
           s.extinct, s.extinct AND s.hours_whole
      FROM islands i
      JOIN tranches t USING (pool_address, started_at)
     CROSS JOIN LATERAL (
         SELECT i.last + $5::INTERVAL <= $3 AS extinct,
                time_bucket('1 hour', i.last) + INTERVAL '1 hour' - $6::INTERVAL <= $3
                    AS hours_whole
     ) s
     GROUP BY i.pool_address, i.started_at, i.last, s.extinct, s.hours_whole
)
INSERT INTO analytic_burst_hours (pool_address, started_at, bucket, protocol, volume_usd)
SELECT pool_address, started_at, time_bucket('1 hour', bucket), $1, SUM(volume)
  FROM tranches
 GROUP BY pool_address, started_at, time_bucket('1 hour', bucket)
