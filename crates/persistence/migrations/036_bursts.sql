-- 036_bursts.sql — how many times each tranche's quoted side changed hands, and the bursts
-- it draws, written by yog-analytic. Apart from the swaps' facts: the published volume does
-- not change. No backfill: measured from the first tranche after this migration.

-- NULL without a mean reserve to divide by or with a mint unresolved, and on the tranches
-- written before.
ALTER TABLE analytic_pool_5min ADD COLUMN rotation NUMERIC;
-- The first tranche with its rotation is when the bursts started being measured.
ALTER TABLE analytic_tranches ADD COLUMN rotations BOOLEAN NOT NULL DEFAULT false;

-- A row not `settled` is rewritten by every cycle; a settled one is final: extinct, and every
-- hour it touches written whole. Its volume is on the facts' base: a pool-hour valued whole.
CREATE TABLE analytic_bursts (
    pool_address          TEXT        NOT NULL,
    started_at            TIMESTAMPTZ NOT NULL,
    protocol              TEXT        NOT NULL,
    ended_at              TIMESTAMPTZ NOT NULL,
    volume_usd            NUMERIC     NOT NULL,
    first_hour_volume_usd NUMERIC     NOT NULL,
    extinct               BOOLEAN     NOT NULL,
    settled               BOOLEAN     NOT NULL CHECK (extinct OR NOT settled),
    PRIMARY KEY (pool_address, started_at)
);
CREATE INDEX analytic_bursts_by_protocol ON analytic_bursts (protocol, started_at DESC);
CREATE INDEX analytic_bursts_unsettled ON analytic_bursts (protocol) WHERE NOT settled;

CREATE TABLE analytic_burst_hours (
    pool_address TEXT        NOT NULL,
    started_at   TIMESTAMPTZ NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    protocol     TEXT        NOT NULL,
    volume_usd   NUMERIC     NOT NULL,
    PRIMARY KEY (pool_address, started_at, bucket),
    FOREIGN KEY (pool_address, started_at)
        REFERENCES analytic_bursts (pool_address, started_at) ON DELETE CASCADE
);
CREATE INDEX analytic_burst_hours_by_protocol ON analytic_burst_hours (protocol, bucket);

GRANT SELECT, INSERT, DELETE ON analytic_bursts, analytic_burst_hours TO yog_analytic;
GRANT SELECT ON analytic_bursts, analytic_burst_hours, analytic_tranches TO yog_api;
