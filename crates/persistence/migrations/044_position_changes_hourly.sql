-- 044_position_changes_hourly.sql — the positions a protocol opened, locked (vesting or
-- permanent), split and closed, counted by yog-analytic per hour. The raw position events are
-- kept without limit: every hour since the first one is filled here, zero included.

CREATE TABLE analytic_protocol_position_changes_hourly (
    protocol        TEXT        NOT NULL,
    bucket          TIMESTAMPTZ NOT NULL,
    opened          BIGINT      NOT NULL,
    vesting_locks   BIGINT      NOT NULL,
    permanent_locks BIGINT      NOT NULL,
    splits          BIGINT      NOT NULL,
    closed          BIGINT      NOT NULL,
    PRIMARY KEY (protocol, bucket)
);

WITH counts AS (
    SELECT time_bucket('1 hour', timestamp) AS bucket, 'opened' AS kind, COUNT(*) AS n
      FROM meteora_damm_v2_create_position_events GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'vesting', COUNT(*)
      FROM meteora_damm_v2_lock_position_events GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'permanent', COUNT(*)
      FROM meteora_damm_v2_permanent_lock_position_events GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'split', COUNT(*)
      FROM meteora_damm_v2_split_position_events GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'closed', COUNT(*)
      FROM meteora_damm_v2_close_position_events GROUP BY 1
)
INSERT INTO analytic_protocol_position_changes_hourly
    (protocol, bucket, opened, vesting_locks, permanent_locks, splits, closed)
SELECT 'meteora_damm_v2', h.bucket,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'opened'), 0),
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'vesting'), 0),
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'permanent'), 0),
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'split'), 0),
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'closed'), 0)
  -- ⚠️ A series, not the counts' hours alone: an hour without any change is a row at 0.
  FROM generate_series((SELECT MIN(bucket) FROM counts), date_trunc('hour', now()),
                       INTERVAL '1 hour') AS h (bucket)
  LEFT JOIN counts c ON c.bucket = h.bucket
 GROUP BY h.bucket;

GRANT SELECT, INSERT, UPDATE ON analytic_protocol_position_changes_hourly TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_create_position_events, meteora_damm_v2_close_position_events,
                meteora_damm_v2_split_position_events
    TO yog_analytic;
GRANT SELECT ON analytic_protocol_position_changes_hourly TO yog_api;
