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

-- The changes of each hour from `first` through `last`, zero included: one count per event.
CREATE FUNCTION meteora_damm_v2_position_changes(first TIMESTAMPTZ, last TIMESTAMPTZ)
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
    SELECT time_bucket('1 hour', timestamp) AS bucket, 'opened' AS kind, COUNT(*) AS n
      FROM meteora_damm_v2_create_position_events
     WHERE timestamp >= first AND timestamp < last + INTERVAL '1 hour' GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'vesting', COUNT(*)
      FROM meteora_damm_v2_lock_position_events
     WHERE timestamp >= first AND timestamp < last + INTERVAL '1 hour' GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'permanent', COUNT(*)
      FROM meteora_damm_v2_permanent_lock_position_events
     WHERE timestamp >= first AND timestamp < last + INTERVAL '1 hour' GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'split', COUNT(*)
      FROM meteora_damm_v2_split_position_events
     WHERE timestamp >= first AND timestamp < last + INTERVAL '1 hour' GROUP BY 1
    UNION ALL
    SELECT time_bucket('1 hour', timestamp), 'closed', COUNT(*)
      FROM meteora_damm_v2_close_position_events
     WHERE timestamp >= first AND timestamp < last + INTERVAL '1 hour' GROUP BY 1
)
SELECT h.bucket,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'opened'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'vesting'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'permanent'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'split'), 0)::BIGINT,
       COALESCE(SUM(c.n) FILTER (WHERE c.kind = 'closed'), 0)::BIGINT
  -- ⚠️ A series, not the counts' hours alone: an hour without any change is a row at 0.
  FROM generate_series(first, last, INTERVAL '1 hour') AS h (bucket)
  LEFT JOIN counts c ON c.bucket = h.bucket
 GROUP BY h.bucket;
END;

INSERT INTO analytic_protocol_position_changes_hourly
    (protocol, bucket, opened, vesting_locks, permanent_locks, splits, closed)
SELECT 'meteora_damm_v2', c.*
  FROM meteora_damm_v2_position_changes(
           date_trunc('hour', LEAST(
               (SELECT MIN(timestamp) FROM meteora_damm_v2_create_position_events),
               (SELECT MIN(timestamp) FROM meteora_damm_v2_lock_position_events),
               (SELECT MIN(timestamp) FROM meteora_damm_v2_permanent_lock_position_events),
               (SELECT MIN(timestamp) FROM meteora_damm_v2_split_position_events),
               (SELECT MIN(timestamp) FROM meteora_damm_v2_close_position_events))),
           date_trunc('hour', now())) AS c;

GRANT SELECT, INSERT, UPDATE ON analytic_protocol_position_changes_hourly TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_create_position_events, meteora_damm_v2_close_position_events,
                meteora_damm_v2_split_position_events
    TO yog_analytic;
GRANT SELECT ON analytic_protocol_position_changes_hourly TO yog_api;
