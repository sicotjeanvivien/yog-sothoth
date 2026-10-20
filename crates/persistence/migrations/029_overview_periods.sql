-- 029_overview_periods.sql — what the Overview reads beyond the hourly facts,
-- written by yog-analytic: the pools born per protocol and hour, and, at each
-- freeze, the active pools and the tokens of each window ending at that hour —
-- counts of distinct pools, which no sum of hours gives.

CREATE TABLE analytic_protocol_births_hourly (
    protocol TEXT        NOT NULL,
    bucket   TIMESTAMPTZ NOT NULL,
    births   BIGINT      NOT NULL,
    PRIMARY KEY (protocol, bucket)
);

-- `bucket` is the last hour of the window; one row per window and frozen hour.
CREATE TABLE analytic_protocol_periods (
    protocol     TEXT        NOT NULL,
    period       TEXT        NOT NULL CHECK (period IN ('24h', '7d', '30d')),
    bucket       TIMESTAMPTZ NOT NULL,
    active_pools BIGINT      NOT NULL,
    PRIMARY KEY (protocol, period, bucket)
);

-- Only the last frozen hour's windows: every row is replaced at each freeze.
CREATE TABLE analytic_token_periods (
    period            TEXT        NOT NULL CHECK (period IN ('24h', '7d', '30d')),
    mint              TEXT        NOT NULL,
    bucket            TIMESTAMPTZ NOT NULL,
    volume_usd        NUMERIC     NOT NULL,
    pools             BIGINT      NOT NULL,
    pool_hours        BIGINT      NOT NULL,
    pool_hours_valued BIGINT      NOT NULL,
    PRIMARY KEY (period, mint)
);

CREATE INDEX analytic_token_periods_by_volume
    ON analytic_token_periods (period, volume_usd DESC, mint);

-- The hours already written, which no cycle rewrites once frozen.
INSERT INTO analytic_protocol_births_hourly (protocol, bucket, births)
SELECT 'meteora_damm_v2', time_bucket('1 hour', timestamp), COUNT(DISTINCT pool_address)
  FROM meteora_damm_v2_initialize_pool_events
 WHERE timestamp >= (SELECT MIN(bucket) FROM analytic_hours)
   AND timestamp <  (SELECT MAX(bucket) FROM analytic_hours) + INTERVAL '1 hour'
 GROUP BY time_bucket('1 hour', timestamp);

GRANT SELECT, INSERT, UPDATE ON analytic_protocol_births_hourly TO yog_analytic;
-- Written once: no UPDATE, so a second write fails in Postgres itself.
GRANT SELECT, INSERT ON analytic_protocol_periods TO yog_analytic;
GRANT SELECT, INSERT, DELETE ON analytic_token_periods TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_initialize_pool_events TO yog_analytic;
GRANT SELECT
    ON analytic_protocol_births_hourly, analytic_protocol_periods, analytic_token_periods
    TO yog_api;
