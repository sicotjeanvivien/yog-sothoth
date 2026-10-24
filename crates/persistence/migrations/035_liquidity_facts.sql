-- 035_liquidity_facts.sql — every liquidity move valued once by yog-analytic, at its
-- pool's price in the tranche — its creation's deposit among them — and their sums per
-- protocol and hour. Apart from the swaps' facts: a pool that only moves liquidity is not
-- an active pool. No backfill: valued from the first tranche after this migration.

-- `amount_usd` is NULL exactly when the move could not be valued.
CREATE TABLE analytic_liquidity_moves (
    signature    TEXT        NOT NULL,
    event_index  INTEGER     NOT NULL,
    timestamp    TIMESTAMPTZ NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    pool_address TEXT        NOT NULL,
    protocol     TEXT        NOT NULL,
    owner        TEXT        NOT NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('create', 'add', 'remove')),
    amount_usd   NUMERIC,
    PRIMARY KEY (signature, event_index, timestamp)
);

SELECT create_hypertable('analytic_liquidity_moves', 'timestamp',
                         chunk_time_interval => INTERVAL '7 days',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_liquidity_moves_by_protocol
    ON analytic_liquidity_moves (protocol, timestamp DESC);
-- ⚠️ A day past the longest window: a chunk is dropped whole, by its end.
SELECT add_retention_policy('analytic_liquidity_moves', INTERVAL '31 days');

-- One row per protocol and written hour, zero included: the first one is when the
-- liquidity started being measured.
CREATE TABLE analytic_protocol_liquidity_hourly (
    protocol     TEXT        NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    created_usd  NUMERIC     NOT NULL,
    added_usd    NUMERIC     NOT NULL,
    removed_usd  NUMERIC     NOT NULL,
    moves        BIGINT      NOT NULL,
    moves_valued BIGINT      NOT NULL,
    PRIMARY KEY (protocol, bucket)
);

-- Written once: no UPDATE, so a second write fails in Postgres itself.
GRANT SELECT, INSERT ON analytic_liquidity_moves TO yog_analytic;
GRANT SELECT, INSERT, UPDATE ON analytic_protocol_liquidity_hourly TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_liquidity_events TO yog_analytic;
GRANT SELECT ON analytic_liquidity_moves, analytic_protocol_liquidity_hourly TO yog_api;
