-- ============================================================
-- yog-sothoth — Initial migration
-- Last updated: April 2026
--
-- Yog-Sothoth is a protocol-centric observer: it subscribes to
-- Meteora programs at the RPC level, discovers pools from the
-- transaction stream, and upserts them into the `pools` table
-- as it observes new ones.
--
-- All event tables (`swap_events`, `liquidity_events`,
-- `position_fee_claims`, `reward_claims`) reference `pool_address`
-- as raw data, without foreign key constraints. Pool dimensions
-- are denormalised into the `pools` table for query convenience,
-- not integrity.
--
-- NOTE — watched_pools allowlist (Phase 1 temporary constraint):
-- Until the indexer runs on an upgraded RPC path (Helius
-- transactionSubscribe, Launchpad, or equivalent gRPC), ingestion
-- is bounded to an allowlist of pools stored in `watched_pools`.
-- ============================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================================
-- pools
-- Registry of pools discovered from the transaction stream.
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
-- ============================================================
CREATE TABLE watched_pools (
    pool_address TEXT        PRIMARY KEY,
    protocol     TEXT        NOT NULL,
    active       BOOLEAN     NOT NULL DEFAULT TRUE,
    added_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    note         TEXT
);

CREATE INDEX idx_watched_pools_active
    ON watched_pools (pool_address)
    WHERE active = TRUE;

-- ============================================================
-- swap_events
-- One row per on-chain Anchor swap event.
--
-- All amount and reserve fields follow the canonical (token_a, token_b)
-- pool ordering. The trader's perspective is recovered by combining
-- `trade_direction` with `amount_a` / `amount_b`.
-- ============================================================
CREATE TABLE swap_events (
    id                 BIGSERIAL,
    pool_address       TEXT           NOT NULL,
    protocol           TEXT           NOT NULL,
    signature          TEXT           NOT NULL,

    -- Pool tokens (canonical order)
    token_a_mint       TEXT           NOT NULL,
    token_b_mint       TEXT           NOT NULL,

    -- Direction and amounts
    trade_direction    TEXT           NOT NULL,  -- 'a_to_b' | 'b_to_a'
    amount_a           BIGINT         NOT NULL,
    amount_b           BIGINT         NOT NULL,

    -- Post-swap pool state
    reserve_a_after    BIGINT         NOT NULL,
    reserve_b_after    BIGINT         NOT NULL,
    next_sqrt_price    NUMERIC(39, 0) NOT NULL,  -- u128 lossless

    -- Fee breakdown
    claiming_fee       BIGINT         NOT NULL,
    protocol_fee       BIGINT         NOT NULL,
    compounding_fee    BIGINT         NOT NULL,
    referral_fee       BIGINT         NOT NULL,
    fee_token_is_a     BOOLEAN        NOT NULL,

    timestamp          TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- liquidity_events
-- ============================================================
CREATE TABLE liquidity_events (
    id                   BIGSERIAL,
    pool_address         TEXT           NOT NULL,
    protocol             TEXT           NOT NULL,
    signature            TEXT           NOT NULL,

    token_a_mint         TEXT           NOT NULL,
    token_b_mint         TEXT           NOT NULL,

    liquidity_event_kind TEXT           NOT NULL,  -- 'add' | 'remove'
    amount_a             BIGINT         NOT NULL,
    amount_b             BIGINT         NOT NULL,
    liquidity_delta      NUMERIC(39, 0) NOT NULL,  -- u128 lossless

    reserve_a_after      BIGINT         NOT NULL,
    reserve_b_after      BIGINT         NOT NULL,

    position             TEXT           NOT NULL,
    owner                TEXT           NOT NULL,

    timestamp            TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- position_fee_claims
-- LP claim of accumulated trading fees on a position.
-- ============================================================
CREATE TABLE position_fee_claims (
    id             BIGSERIAL,
    pool_address   TEXT        NOT NULL,
    protocol       TEXT        NOT NULL,
    signature      TEXT        NOT NULL,

    position       TEXT        NOT NULL,
    owner          TEXT        NOT NULL,

    fee_a_claimed  BIGINT      NOT NULL,
    fee_b_claimed  BIGINT      NOT NULL,

    timestamp      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- reward_claims
-- LP claim of farming rewards (separate token from the pool's pair).
-- ============================================================
CREATE TABLE reward_claims (
    id            BIGSERIAL,
    pool_address  TEXT        NOT NULL,
    protocol      TEXT        NOT NULL,
    signature     TEXT        NOT NULL,

    position      TEXT        NOT NULL,
    owner         TEXT        NOT NULL,

    mint_reward   TEXT        NOT NULL,
    reward_index  SMALLINT    NOT NULL,
    total_reward  BIGINT      NOT NULL,

    timestamp     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- ============================================================
-- Hypertables
-- ============================================================
SELECT create_hypertable('swap_events',         'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('liquidity_events',    'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('position_fee_claims', 'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('reward_claims',       'timestamp', chunk_time_interval => INTERVAL '7 days');

-- ============================================================
-- Indexes for common query patterns
-- ============================================================
CREATE INDEX ON swap_events         (pool_address, timestamp DESC);
CREATE INDEX ON liquidity_events    (pool_address, timestamp DESC);
CREATE INDEX ON position_fee_claims (pool_address, timestamp DESC);
CREATE INDEX ON reward_claims       (pool_address, timestamp DESC);

CREATE INDEX ON swap_events         (protocol, timestamp DESC);
CREATE INDEX ON liquidity_events    (protocol, timestamp DESC);

-- Per-position queries: useful for LP dashboards.
CREATE INDEX ON liquidity_events    (position, timestamp DESC);
CREATE INDEX ON position_fee_claims (position, timestamp DESC);
CREATE INDEX ON reward_claims       (position, timestamp DESC);

-- Idempotency: replay-safe inserts via ON CONFLICT DO NOTHING.
CREATE UNIQUE INDEX ON swap_events         (signature, timestamp);
CREATE UNIQUE INDEX ON liquidity_events    (signature, timestamp);
CREATE UNIQUE INDEX ON position_fee_claims (signature, timestamp);
CREATE UNIQUE INDEX ON reward_claims       (signature, timestamp);

-- ============================================================
-- Compression (after 7 days)
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
ALTER TABLE position_fee_claims SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
ALTER TABLE reward_claims SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

SELECT add_compression_policy('swap_events',         INTERVAL '7 days');
SELECT add_compression_policy('liquidity_events',    INTERVAL '7 days');
SELECT add_compression_policy('position_fee_claims', INTERVAL '7 days');
SELECT add_compression_policy('reward_claims',       INTERVAL '7 days');

-- ============================================================
-- Retention (30 days for raw data, Phase 1 / MVP)
-- ============================================================
SELECT add_retention_policy('swap_events',         INTERVAL '30 days');
SELECT add_retention_policy('liquidity_events',    INTERVAL '30 days');
SELECT add_retention_policy('position_fee_claims', INTERVAL '30 days');
SELECT add_retention_policy('reward_claims',       INTERVAL '30 days');