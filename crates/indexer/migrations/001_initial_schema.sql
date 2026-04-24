-- ============================================================
-- yog-sothoth — Initial migration
-- Last updated: April 2026
--
-- Yog-Sothoth is a protocol-centric observer: it subscribes to
-- Meteora programs at the RPC level, discovers pools from the
-- transaction stream, and upserts them into the `pools` table
-- as it observes new ones.
--
-- Events (`swap_events`, `liquidity_events`) and snapshots
-- (`pool_metrics`) reference `pool_address` as raw data, without
-- foreign key constraints. Pool dimensions are denormalised into
-- the `pools` table for query convenience, not integrity.
--
-- NOTE — watched_pools allowlist (Phase 1 temporary constraint):
-- Until the indexer runs on an upgraded RPC path (Helius
-- transactionSubscribe, Launchpad, or equivalent gRPC), ingestion
-- is bounded to an allowlist of pools stored in `watched_pools`.
-- The protocol-centric architecture is preserved — the allowlist
-- is applied as a filter in the ingestion pipeline, not as a
-- return to static configuration. Lifting the constraint later
-- is a matter of disabling the filter.
-- ============================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================================
-- pools
-- Registry of pools discovered from the transaction stream.
-- Upserted on every parsed event:
--   - first observation inserts the full row
--   - subsequent observations refresh last_seen_at
--
-- Mint ordering follows the stable pool convention
-- (sorted by raw pubkey bytes — see crate::domain::Pool).
-- ============================================================
CREATE TABLE pools (
    pool_address  TEXT        PRIMARY KEY,
    protocol      TEXT        NOT NULL,
    token_a_mint  TEXT        NOT NULL,
    token_b_mint  TEXT        NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pools_protocol     ON pools (protocol);
CREATE INDEX idx_pools_last_seen_at ON pools (last_seen_at DESC);

-- ============================================================
-- watched_pools
-- Allowlist of pools that the ingestion pipeline is permitted
-- to process. Loaded at indexer startup into an in-memory set
-- used by the WatchedPoolFilter.
--
-- This table is decoupled from `pools`:
--   - No foreign key — a watched pool may not yet have been
--     observed (added_at < first_seen_at is valid).
--   - Deactivation uses the `active` flag rather than row
--     deletion, to preserve history and allow reactivation
--     without re-selecting.
--
-- The `note` column is free-form annotation (selection rationale,
-- edge-case marker, etc.) — useful for future-self context when
-- revisiting the selection.
--
-- Schema anticipates v0.3 user-managed watchlists: a `user_id`
-- column will be added then, with the current rows migrating
-- to a system-owned user or a dedicated default tier.
-- ============================================================
CREATE TABLE watched_pools (
    pool_address TEXT        PRIMARY KEY,
    protocol     TEXT        NOT NULL,
    active       BOOLEAN     NOT NULL DEFAULT TRUE,
    added_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    note         TEXT
);

-- Partial index: filter queries only care about active rows.
-- Keeps the index small and fast as the table grows over time
-- with deactivated entries.
CREATE INDEX idx_watched_pools_active
    ON watched_pools (pool_address)
    WHERE active = TRUE;

-- ============================================================
-- swap_events
-- One row per swap transaction.
--
-- `reserve_in_*` and `reserve_out_*` follow the direction of the swap,
-- not the stable (token_a, token_b) convention — see SwapEvent docs.
-- ============================================================
CREATE TABLE swap_events (
    id                  BIGSERIAL,
    pool_address        TEXT         NOT NULL,
    protocol            TEXT         NOT NULL,
    signature           TEXT         NOT NULL,
    token_a_mint        TEXT         NOT NULL,
    token_b_mint        TEXT         NOT NULL,
    token_in_mint       TEXT         NOT NULL,
    token_out_mint      TEXT         NOT NULL,
    amount_in           BIGINT       NOT NULL,
    amount_out          BIGINT       NOT NULL,
    reserve_in_before   BIGINT       NOT NULL,
    reserve_out_before  BIGINT       NOT NULL,
    reserve_in_after    BIGINT       NOT NULL,
    reserve_out_after   BIGINT       NOT NULL,
    fee_bps             INTEGER,
    fee_amount          BIGINT,
    timestamp           TIMESTAMPTZ  NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- liquidity_events
-- One row per add or remove liquidity transaction.
--
-- `amount_a` / `amount_b` are aligned with `token_a_mint` / `token_b_mint`
-- (stable pool convention — see LiquidityEvent docs).
-- ============================================================
CREATE TABLE liquidity_events (
    id                   BIGSERIAL,
    pool_address         TEXT         NOT NULL,
    protocol             TEXT         NOT NULL,
    signature            TEXT         NOT NULL,
    token_a_mint         TEXT         NOT NULL,
    token_b_mint         TEXT         NOT NULL,
    liquidity_event_kind TEXT         NOT NULL,  -- 'add' | 'remove'
    amount_a             BIGINT       NOT NULL,
    amount_b             BIGINT       NOT NULL,
    timestamp            TIMESTAMPTZ  NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- pool_metrics
-- Time series — one row per indexed event.
-- Represents the pool state after each event.
--
-- Reserves follow the stable (token_a, token_b) convention.
-- Primary key: (pool_address, signature, timestamp)
-- Rationale: two transactions can land in the same slot. The
-- signature guarantees true uniqueness.
-- ============================================================
CREATE TABLE pool_metrics (
    pool_address      TEXT           NOT NULL,
    signature         TEXT           NOT NULL,

    -- Reserves (stable order)
    reserve_a         BIGINT         NOT NULL,
    reserve_b         BIGINT         NOT NULL,

    -- Price as Q64 fixed-point (encoded u128 integer).
    -- NUMERIC(39, 0) guarantees lossless precision for a u128.
    -- Rust side: cast u128 -> BigDecimal before INSERT.
    price_q64         NUMERIC(39, 0) NOT NULL,

    -- Swap quality metrics
    price_impact_bps  INTEGER,  -- NULL for non-swap events
    imbalance_bps     INTEGER,

    -- Fees - distinct semantics from swap_events.fee_bps
    -- current_fee_bps  : fee rate in effect at the time of the event
    -- fees_collected_* : absolute fee amount collected on this event
    current_fee_bps   INTEGER,  -- DAMM v2 only
    fees_collected_a  BIGINT,
    fees_collected_b  BIGINT,

    -- Volume - NULL if event is add/remove liquidity (not a swap)
    volume_a          BIGINT,
    volume_b          BIGINT,

    -- DLMM-specific (NULL for DAMM v1 / v2)
    active_bin_id     INTEGER,
    bin_step          SMALLINT,

    timestamp         TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (pool_address, signature, timestamp)
);

-- ============================================================
-- Convert to TimescaleDB hypertables
-- chunk_time_interval = 7 days: reasonable trade-off for a Solana
-- feed (moderate volume, queries over sliding windows of a few days)
-- ============================================================
SELECT create_hypertable('swap_events',      'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('liquidity_events', 'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('pool_metrics',     'timestamp', chunk_time_interval => INTERVAL '7 days');

-- ============================================================
-- Indexes for common query patterns
-- ============================================================
CREATE INDEX ON swap_events      (pool_address, timestamp DESC);
CREATE INDEX ON liquidity_events (pool_address, timestamp DESC);
CREATE INDEX ON pool_metrics     (pool_address, timestamp DESC);

CREATE INDEX ON swap_events      (protocol, timestamp DESC);
CREATE INDEX ON liquidity_events (protocol, timestamp DESC);

-- Idempotency: (signature, timestamp) must be unique.
-- Allows INSERT ... ON CONFLICT DO NOTHING on the Rust indexer side
-- to replay blocks without creating duplicates.
CREATE UNIQUE INDEX ON swap_events      (signature, timestamp);
CREATE UNIQUE INDEX ON liquidity_events (signature, timestamp);

-- ============================================================
-- Automatic compression after 7 days.
-- compress_segmentby = 'pool_address': queries are almost always
-- filtered by pool - segmenting here optimises decompression.
-- ============================================================
ALTER TABLE swap_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

ALTER TABLE liquidity_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

ALTER TABLE pool_metrics SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

SELECT add_compression_policy('swap_events',      INTERVAL '7 days');
SELECT add_compression_policy('liquidity_events', INTERVAL '7 days');
SELECT add_compression_policy('pool_metrics',     INTERVAL '7 days');

-- ============================================================
-- Retention policy
-- 30 days for raw data in Phase 1 / MVP.
-- Revisit in production: 90 days minimum if historical API access
-- is monetised; consider tiered retention (raw 30d, aggregates infinity).
-- ============================================================
SELECT add_retention_policy('swap_events',      INTERVAL '30 days');
SELECT add_retention_policy('liquidity_events', INTERVAL '30 days');
SELECT add_retention_policy('pool_metrics',     INTERVAL '30 days');

-- ============================================================
-- Continuous aggregate - hourly view
-- Avoids expensive on-the-fly aggregations on every dashboard query.
-- Used for: 24h volume, cumulative fees, OHLC prices, TVL snapshots.
-- ============================================================
CREATE MATERIALIZED VIEW pool_metrics_1h
WITH (timescaledb.continuous) AS
SELECT
    pool_address,
    time_bucket('1 hour', timestamp) AS bucket,

    -- Price: last value in the window
    last(price_q64, timestamp) AS price_q64_close,

    -- Reserves: last known state in the window
    last(reserve_a, timestamp) AS reserve_a,
    last(reserve_b, timestamp) AS reserve_b,

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
GROUP BY pool_address, bucket
WITH NO DATA;

-- Refresh policy: 1 day back -> 1 hour ago, every hour.
-- TimescaleDB only recomputes recently modified buckets.
SELECT add_continuous_aggregate_policy(
    'pool_metrics_1h',
    start_offset      => INTERVAL '1 day',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);

-- Hourly aggregate retention: 1 year.
-- Aggregates are far lighter than raw data - keeping them long-term
-- is cheap and supports monetisation of historical API access.
SELECT add_retention_policy('pool_metrics_1h', INTERVAL '365 days');