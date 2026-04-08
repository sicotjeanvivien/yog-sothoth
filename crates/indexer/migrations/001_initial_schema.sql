-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Watched pools — pools configured for indexing
CREATE TABLE watched_pools (
    address         TEXT        PRIMARY KEY,
    protocol        TEXT        NOT NULL,   -- 'damm_v2' | 'dlmm' | 'damm_v1'
    token_a_mint    TEXT        NOT NULL,
    token_b_mint    TEXT        NOT NULL,
    token_a_decimals SMALLINT   NOT NULL,
    token_b_decimals SMALLINT   NOT NULL,
    added_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Swap events — one row per swap transaction
CREATE TABLE swap_events (
    id              BIGSERIAL,
    pool_address    TEXT        NOT NULL REFERENCES watched_pools(address),
    signature       TEXT        NOT NULL,
    token_in        TEXT        NOT NULL,
    token_out       TEXT        NOT NULL,
    amount_in       NUMERIC     NOT NULL,   -- native units
    amount_out      NUMERIC     NOT NULL,   -- native units
    fee_bps         INTEGER,                -- nullable — protocol-specific
    timestamp       TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- Liquidity events — one row per add/remove transaction
CREATE TABLE liquidity_events (
    id              BIGSERIAL,
    pool_address    TEXT        NOT NULL REFERENCES watched_pools(address),
    signature       TEXT        NOT NULL,
    event_type      TEXT        NOT NULL,   -- 'add' | 'remove'
    amount_a        NUMERIC     NOT NULL,   -- native units
    amount_b        NUMERIC     NOT NULL,   -- native units
    timestamp       TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- Pool metrics — time-series, one row per transaction
CREATE TABLE pool_metrics (
    pool_address    TEXT        NOT NULL REFERENCES watched_pools(address),
    -- Common fields
    reserve_a       NUMERIC     NOT NULL,   -- native units
    reserve_b       NUMERIC     NOT NULL,   -- native units
    price_q64       NUMERIC     NOT NULL,   -- Q64 fixed-point
    price_impact_bps INTEGER,               -- basis points
    imbalance_bps   INTEGER,                -- basis points
    -- DAMM v2 specific — NULL for other protocols
    fee_bps         INTEGER,                -- dynamic fee at time of event
    -- DLMM specific — NULL for other protocols
    active_bin_id   INTEGER,
    timestamp       TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (pool_address, timestamp)
);

-- Convert pool_metrics to a TimescaleDB hypertable
SELECT create_hypertable('pool_metrics', 'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('swap_events', 'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('liquidity_events', 'timestamp', chunk_time_interval => INTERVAL '7 days');

-- Automatic compression after 7 days
ALTER TABLE pool_metrics SET (
    timescaledb.compress,
    timescaledb.compress_orderby = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

ALTER TABLE swap_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

ALTER TABLE liquidity_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

SELECT add_compression_policy('pool_metrics', INTERVAL '7 days');
SELECT add_compression_policy('swap_events', INTERVAL '7 days');
SELECT add_compression_policy('liquidity_events', INTERVAL '7 days');

-- Retention policy — 30 days
SELECT add_retention_policy('pool_metrics', INTERVAL '30 days');
SELECT add_retention_policy('swap_events', INTERVAL '30 days');
SELECT add_retention_policy('liquidity_events', INTERVAL '30 days');

-- Indexes for common query patterns
CREATE INDEX ON swap_events (pool_address, timestamp DESC);
CREATE INDEX ON liquidity_events (pool_address, timestamp DESC);
CREATE INDEX ON pool_metrics (pool_address, timestamp DESC);

-- Idempotency indexes — signature + timestamp must be unique
CREATE UNIQUE INDEX ON swap_events (signature, timestamp);
CREATE UNIQUE INDEX ON liquidity_events (signature, timestamp);