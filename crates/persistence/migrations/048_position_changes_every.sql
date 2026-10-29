-- 048_position_changes_every.sql — the position changes of DAMM v2 counted by any step: the hour
-- yog-analytic writes, the 5-minute tranche the API reads for the last hour. One count, two steps.

CREATE FUNCTION meteora_damm_v2_position_changes_every(first TIMESTAMPTZ, last TIMESTAMPTZ, width INTERVAL)
RETURNS TABLE (
    bucket          TIMESTAMPTZ,
    opened          BIGINT,
    vesting_locks   BIGINT,
    permanent_locks BIGINT,
    splits          BIGINT,
    closed          BIGINT
)
LANGUAGE sql STABLE
BEGIN ATOMIC
WITH counts AS (
    SELECT time_bucket(width, timestamp) AS bucket, 'opened' AS kind, COUNT(*) AS n
      FROM meteora_damm_v2_create_position_events
     WHERE timestamp >= first AND timestamp < last + width GROUP BY 1
    UNION ALL
    SELECT time_bucket(width, timestamp), 'vesting', COUNT(*)
      FROM meteora_damm_v2_lock_position_events
     WHERE timestamp >= first AND timestamp < last + width GROUP BY 1
    UNION ALL
    SELECT time_bucket(width, timestamp), 'permanent', COUNT(*)
      FROM meteora_damm_v2_permanent_lock_position_events
     WHERE timestamp >= first AND timestamp < last + width GROUP BY 1
    UNION ALL
    SELECT time_bucket(width, timestamp), 'split', COUNT(*)
      FROM meteora_damm_v2_split_position_events
     WHERE timestamp >= first AND timestamp < last + width GROUP BY 1
    UNION ALL
    SELECT time_bucket(width, timestamp), 'closed', COUNT(*)
      FROM meteora_damm_v2_close_position_events
     WHERE timestamp >= first AND timestamp < last + width GROUP BY 1
)
SELECT b.bucket,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'opened'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'vesting'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'permanent'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'split'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'closed'), 0)::BIGINT
  -- ⚠️ A series, not the counts' buckets alone: a bucket without any change is a row at 0.
  FROM generate_series(first, last, width) AS b (bucket)
  LEFT JOIN counts c ON c.bucket = b.bucket
 GROUP BY b.bucket;
END;

CREATE OR REPLACE FUNCTION meteora_damm_v2_position_changes(first TIMESTAMPTZ, last TIMESTAMPTZ)
RETURNS TABLE (
    bucket          TIMESTAMPTZ,
    opened          BIGINT,
    vesting_locks   BIGINT,
    permanent_locks BIGINT,
    splits          BIGINT,
    closed          BIGINT
)
LANGUAGE sql STABLE
BEGIN ATOMIC
SELECT * FROM meteora_damm_v2_position_changes_every(first, last, INTERVAL '1 hour');
END;
