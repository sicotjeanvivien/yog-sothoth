-- ============================================================
-- yog-sothoth — Initial migration
-- Last updated: April 2026
-- ============================================================

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================================
-- watched_pools
-- Registry of pools configured for indexing.
-- Populated by the dashboard (add / remove pools).
-- ============================================================
CREATE TABLE watched_pools (
    address TEXT PRIMARY KEY,
    protocol TEXT NOT NULL, -- 'damm_v2' | 'dlmm' | 'damm_v1'
    token_a_mint TEXT NOT NULL,
    token_b_mint TEXT NOT NULL,
    token_a_decimals SMALLINT NOT NULL,
    token_b_decimals SMALLINT NOT NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- swap_events
-- One row per swap transaction.
-- ============================================================
CREATE TABLE swap_events (
    id BIGSERIAL,
    pool_address TEXT NOT NULL REFERENCES watched_pools (address),
    signature TEXT NOT NULL,
    token_in TEXT NOT NULL,
    token_out TEXT NOT NULL,
    amount_in NUMERIC NOT NULL, -- native units (before decimals)
    amount_out NUMERIC NOT NULL, -- native units (before decimals)
    fee_bps INTEGER, -- fee applied to this specific swap (nullable — protocol-specific)
    timestamp TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- liquidity_events
-- One row per add or remove liquidity transaction.
-- ============================================================
CREATE TABLE liquidity_events (
    id BIGSERIAL,
    pool_address TEXT NOT NULL REFERENCES watched_pools (address),
    signature TEXT NOT NULL,
    liquidity_event_kind TEXT NOT NULL, -- 'add' | 'remove'
    amount_a NUMERIC NOT NULL, -- native units
    amount_b NUMERIC NOT NULL, -- native units
    timestamp TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- pool_metrics
-- Time series — one row per indexed transaction.
-- Represents the pool state after each event.
--
-- Primary key: (pool_address, signature, timestamp)
-- Rationale: two transactions can land in the same block (same
-- second). The signature guarantees true uniqueness.
-- ============================================================
CREATE TABLE pool_metrics (
    pool_address        TEXT        NOT NULL REFERENCES watched_pools(address),
    signature           TEXT        NOT NULL,   -- Solana tx that triggered this state update

-- Reserves
reserve_a NUMERIC NOT NULL, -- native units token A
reserve_b NUMERIC NOT NULL, -- native units token B

-- Price
-- Stored as Q64 fixed-point (encoded u128 integer).
-- NUMERIC(39, 0) guarantees lossless precision for a u128.
-- Rust side: cast u128 → BigDecimal before INSERT.
price_q64 NUMERIC(39, 0) NOT NULL,

-- Swap quality metrics
price_impact_bps INTEGER, -- price impact of the swap in basis points (NULL for non-swap events)
imbalance_bps INTEGER, -- reserve imbalance in basis points

-- Fees — distinct semantics from swap_events.fee_bps
-- current_fee_bps  : dynamic fee rate in effect at the time of the event
-- fees_collected_* : absolute fee amount collected on this event
current_fee_bps INTEGER, -- DAMM v2: current dynamic fee rate (NULL for other protocols)
fees_collected_a NUMERIC, -- fee amount token A on this event (native units)
fees_collected_b NUMERIC, -- fee amount token B on this event (native units)

-- Volume — amounts traded on this event, used for aggregation
-- NULL if the event is an add/remove liquidity (not a swap)
volume_a NUMERIC, -- native units token A traded
volume_b NUMERIC, -- native units token B traded

-- DLMM-specific (NULL for DAMM v2 and DAMM v1)
active_bin_id       INTEGER,                -- active bin ID at the time of the event
    -- bin_step is constant per DLMM pool — stored here to avoid an RPC call
    -- from the frontend when recalculating bin prices
    bin_step            SMALLINT,               -- bin step in basis points

    timestamp           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (pool_address, signature, timestamp)
);

-- ============================================================
-- Convert to TimescaleDB hypertables
-- chunk_time_interval = 7 days: reasonable trade-off for a Solana
-- feed (moderate volume, queries over sliding windows of a few days)
-- ============================================================
SELECT create_hypertable (
        'pool_metrics', 'timestamp', chunk_time_interval => INTERVAL '7 days'
    );

SELECT create_hypertable (
        'swap_events', 'timestamp', chunk_time_interval => INTERVAL '7 days'
    );

SELECT create_hypertable (
        'liquidity_events', 'timestamp', chunk_time_interval => INTERVAL '7 days'
    );

-- ============================================================
-- Automatic compression after 7 days
-- compress_segmentby = 'pool_address': queries are almost always
-- filtered by pool — segmenting here optimises decompression.
-- ============================================================
ALTER TABLE pool_metrics
SET (
        timescaledb.compress,
        timescaledb.compress_orderby = 'timestamp DESC',
        timescaledb.compress_segmentby = 'pool_address'
    );

ALTER TABLE swap_events
SET (
        timescaledb.compress,
        timescaledb.compress_orderby = 'timestamp DESC',
        timescaledb.compress_segmentby = 'pool_address'
    );

ALTER TABLE liquidity_events
SET (
        timescaledb.compress,
        timescaledb.compress_orderby = 'timestamp DESC',
        timescaledb.compress_segmentby = 'pool_address'
    );

SELECT add_compression_policy ( 'pool_metrics', INTERVAL '7 days' );

SELECT add_compression_policy ( 'swap_events', INTERVAL '7 days' );

SELECT add_compression_policy ( 'liquidity_events', INTERVAL '7 days' );

-- ============================================================
-- Retention policy
-- 30 days for raw data in Phase 1 / MVP.
-- Revisit in production: 90 days minimum if historical API access
-- is monetised; consider tiered retention (raw 30d, aggregates ∞).
-- ============================================================
SELECT add_retention_policy ( 'pool_metrics', INTERVAL '30 days' );

SELECT add_retention_policy ( 'swap_events', INTERVAL '30 days' );

SELECT add_retention_policy ( 'liquidity_events', INTERVAL '30 days' );

-- ============================================================
-- Indexes for common query patterns
-- ============================================================
CREATE INDEX ON swap_events (pool_address, timestamp DESC);

CREATE INDEX ON liquidity_events (pool_address, timestamp DESC);

CREATE INDEX ON pool_metrics (pool_address, timestamp DESC);

-- Idempotency: signature + timestamp must be unique
-- Allows INSERT ... ON CONFLICT DO NOTHING on the Rust indexer side
-- to replay blocks without creating duplicates.
CREATE UNIQUE INDEX ON swap_events (signature, timestamp);

CREATE UNIQUE INDEX ON liquidity_events (signature, timestamp);

-- ============================================================
-- Continuous aggregate — hourly view
-- Computed automatically by TimescaleDB over time.
-- Avoids expensive on-the-fly aggregations on every dashboard query.
-- Used for: 24h volume, cumulative fees, OHLC prices, TVL snapshots.
--
-- Enable in Phase 3 (dashboard + alerts).
-- Requires pool_metrics to be populated (Phase 1 → 2).
-- ============================================================

-- Hourly view: last known value + aggregates per pool
CREATE MATERIALIZED VIEW pool_metrics_1h
WITH (timescaledb.continuous) AS
SELECT
    pool_address,
    time_bucket ('1 hour', timestamp) AS bucket,

-- Price: last value in the window
last (price_q64, timestamp) AS price_q64_close,

-- Reserves: last known state in the window
last (reserve_a, timestamp) AS reserve_a,
last (reserve_b, timestamp) AS reserve_b,

-- Average imbalance over the window
avg(imbalance_bps) AS avg_imbalance_bps,

-- Cumulative volume over the window (NULL if no swap)
sum(volume_a) AS volume_a,
sum(volume_b) AS volume_b,

-- Cumulative fees over the window
sum(fees_collected_a) AS fees_collected_a,
sum(fees_collected_b) AS fees_collected_b,

-- Event count (activity proxy)
count(*) AS event_count
FROM pool_metrics
GROUP BY
    pool_address,
    bucket
WITH
    NO DATA;

-- Refresh policy: sliding window 2h → now
-- TimescaleDB only recomputes recently modified buckets.
SELECT
    add_continuous_aggregate_policy (
        'pool_metrics_1h',
        start_offset => INTERVAL '2 hours',
        end_offset => INTERVAL '5 minutes',
        schedule_interval => INTERVAL '5 minutes'
    );

-- Hourly aggregate retention: 1 year
-- Aggregates are far lighter than raw data — keeping them long-term
-- is cheap and supports monetisation of historical API access.
SELECT add_retention_policy ( 'pool_metrics_1h', INTERVAL '365 days' );