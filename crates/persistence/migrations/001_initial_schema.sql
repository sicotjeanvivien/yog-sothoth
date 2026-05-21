-- ============================================================================
-- 001_initial_schema.sql — yog-sothoth baseline schema
-- ============================================================================
-- Consolidated baseline for v0.1. Combines the historical migrations 001-004
-- into a single applicable unit: pools and event tables, the pool_current_state
-- read model, the network_status singleton, and the token enrichment tables.
--
-- Convention going forward: each migration that creates a table also emits
-- its GRANT statements at the end of the relevant section. setup_roles.sql
-- only declares roles and their structural privileges (schema ownership,
-- default privileges); table-specific grants live next to the table that
-- needs them, in the migration that creates it.
--
-- NOTE on watched_pools (Phase 1 temporary constraint):
-- Until the indexer runs on an upgraded RPC path (Helius transactionSubscribe
-- or equivalent), ingestion is bounded to an allowlist of pools stored in
-- `watched_pools`. The day we lift the allowlist, the table stays — it just
-- stops being read by the indexer's startup filter.
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;


-- ============================================================================
-- pools — registry of pools discovered from the transaction stream
-- ============================================================================
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

GRANT SELECT, INSERT, UPDATE ON pools TO yog_indexer;
GRANT SELECT                 ON pools TO yog_api, yog_context;


-- ============================================================================
-- watched_pools — startup allowlist (Phase 1)
-- ============================================================================
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

GRANT SELECT ON watched_pools TO yog_indexer, yog_api;


-- ============================================================================
-- swap_events — one row per on-chain Anchor swap event
--
-- All amount and reserve fields follow the canonical (token_a, token_b)
-- pool ordering. The trader's perspective is recovered by combining
-- `trade_direction` with `amount_a` / `amount_b`.
-- ============================================================================
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


-- ============================================================================
-- liquidity_events — add / remove liquidity events
-- ============================================================================
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


-- ============================================================================
-- position_fee_claims — LP claim of accumulated trading fees on a position
-- ============================================================================
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


-- ============================================================================
-- reward_claims — LP claim of farming rewards (separate token from the pair)
-- ============================================================================
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


-- ============================================================================
-- Hypertables — event tables become time-partitioned
-- ============================================================================
SELECT create_hypertable('swap_events',         'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('liquidity_events',    'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('position_fee_claims', 'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('reward_claims',       'timestamp', chunk_time_interval => INTERVAL '7 days');


-- ============================================================================
-- Indexes for common query patterns
-- ============================================================================
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


-- ============================================================================
-- Compression (after 7 days)
-- ============================================================================
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


-- ============================================================================
-- Retention (30 days for raw data, Phase 1 / MVP)
-- ============================================================================
SELECT add_retention_policy('swap_events',         INTERVAL '30 days');
SELECT add_retention_policy('liquidity_events',    INTERVAL '30 days');
SELECT add_retention_policy('position_fee_claims', INTERVAL '30 days');
SELECT add_retention_policy('reward_claims',       INTERVAL '30 days');


-- ============================================================================
-- Grants on the event tables
-- ============================================================================
-- The indexer writes events and reads them back for the upsert of
-- pool_current_state (see below). yog_api reads everything.
GRANT SELECT, INSERT, UPDATE
    ON swap_events, liquidity_events, position_fee_claims, reward_claims
    TO yog_indexer;

GRANT SELECT
    ON swap_events, liquidity_events, position_fee_claims, reward_claims
    TO yog_api;

-- Sequences behind BIGSERIAL columns.
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_indexer;


-- ============================================================================
-- pool_current_state — CQRS read model: latest known state of each pool
--
-- Maintained event-driven by the indexer: every persisted swap or liquidity
-- event triggers an upsert into this table. The table is intentionally a
-- regular relation (not a Postgres MATERIALIZED VIEW, not a Timescale
-- continuous aggregate) because we need O(1) lookups by pool_address and
-- we maintain the state ourselves from the event stream.
--
-- Replay safety: upserts use a stale-write guard (WHERE current.last_event_at
-- < EXCLUDED.last_event_at) so reprocessing old events never overwrites a
-- newer state. See PgPoolCurrentStateRepository::upsert.
-- ============================================================================
CREATE TABLE pool_current_state (
    pool_address       TEXT PRIMARY KEY
                       REFERENCES pools(pool_address) ON DELETE CASCADE,
    protocol           TEXT NOT NULL,

    -- Last event of any kind that touched this pool
    last_event_at      TIMESTAMPTZ NOT NULL,
    last_event_kind    TEXT        NOT NULL,
    last_signature    TEXT         NOT NULL,

    -- Canonical reserves (token_a, token_b ordering as established in pools)
    reserve_a          BIGINT NOT NULL,
    reserve_b          BIGINT NOT NULL,

    -- Price proxy: sqrt_price is updated by swap events only
    last_sqrt_price    NUMERIC(39, 0),
    last_swap_at       TIMESTAMPTZ,

    -- Liquidity (L): updated by liquidity events only
    liquidity          NUMERIC(39, 0),
    last_liquidity_at  TIMESTAMPTZ,

    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT pool_current_state_event_kind_valid
        CHECK (last_event_kind IN ('swap', 'liquidity_add', 'liquidity_remove'))
);

COMMENT ON TABLE  pool_current_state IS
    'Per-pool projection of the latest known on-chain state, maintained by the indexer.';
COMMENT ON COLUMN pool_current_state.last_event_kind IS
    'Kind of the most recent event applied: swap | liquidity_add | liquidity_remove.';
COMMENT ON COLUMN pool_current_state.last_sqrt_price IS
    'Last observed sqrt_price (Q64.64 fixed-point as NUMERIC). NULL until first swap.';
COMMENT ON COLUMN pool_current_state.liquidity IS
    'Last observed liquidity L. NULL until first liquidity event.';

CREATE INDEX idx_pool_current_state_protocol
    ON pool_current_state (protocol);
CREATE INDEX idx_pool_current_state_last_event_at
    ON pool_current_state (last_event_at DESC);

GRANT SELECT, INSERT, UPDATE ON pool_current_state TO yog_indexer;
GRANT SELECT                 ON pool_current_state TO yog_api;


-- ============================================================================
-- network_status — singleton snapshot of the indexer's link to Solana
--
-- Not a time series: it answers "what is the state right now" for the
-- dashboard sidebar's "Solana Live" panel, nothing more. Historical health
-- metrics (latency over time, gaps) are Prometheus's job.
--
-- The indexer overwrites the single row every ~15s with the latest observed
-- slot and the round-trip latency of the getSlot call.
-- ============================================================================
CREATE TABLE network_status (
    -- Singleton guard: the CHECK constraint allows only id = 1, so the
    -- table can never hold more than one row. Writers use
    -- INSERT ... ON CONFLICT (id) DO UPDATE.
    id              SMALLINT    PRIMARY KEY DEFAULT 1
                                CHECK (id = 1),

    -- Latest Solana slot observed by the indexer.
    -- Slots are u64 on-chain; stored as BIGINT (cast at the edges).
    slot            BIGINT      NOT NULL,

    -- Round-trip latency of the getSlot RPC call, in milliseconds.
    rpc_latency_ms  INTEGER     NOT NULL,

    -- When the indexer recorded this snapshot (server-side wall clock).
    observed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the singleton row so the very first indexer write is a plain UPDATE
-- path and the API never has to handle an empty table.
INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
VALUES (1, 0, 0, now());

GRANT SELECT, INSERT, UPDATE ON network_status TO yog_indexer;
GRANT SELECT                 ON network_status TO yog_api;


-- ============================================================================
-- token_metadata — one row per mint, near-immutable reference data
--
-- Populated by the yog-context daemon's metadata worker (Helius DAS).
-- NOT a hypertable: slow-changing reference data, one row per mint.
-- ============================================================================
CREATE TABLE token_metadata (
    -- The SPL mint address (base58). Primary key — one row per mint.
    mint              TEXT        PRIMARY KEY,

    -- Symbol and name. NULLABLE on purpose: some tokens (very old, or
    -- raw launches) carry no Metaplex metadata. DAS still returns
    -- decimals in that case, so the row is kept with name/symbol null.
    symbol            TEXT,
    name              TEXT,

    -- Token decimal precision. NOT NULL — DAS always provides it.
    decimals          SMALLINT    NOT NULL,

    -- Logo URI as returned by DAS. May be an ipfs:// URI — stored
    -- verbatim, the frontend resolves it.
    logo_uri          TEXT,

    -- Which source produced this row. A single value for now
    -- ('helius_das'), kept explicit for future-proofing and debug.
    metadata_source   TEXT        NOT NULL DEFAULT 'helius_das',

    -- When the row was first fetched, and when it was last refreshed.
    fetched_at        TIMESTAMPTZ NOT NULL,
    last_refresh_at   TIMESTAMPTZ NOT NULL
);

-- Supports "least recently refreshed" scans, if a refresh policy is
-- added later.
CREATE INDEX idx_token_metadata_last_refresh
    ON token_metadata (last_refresh_at);

GRANT SELECT, INSERT, UPDATE ON token_metadata TO yog_context;
GRANT SELECT                 ON token_metadata TO yog_api;


-- ============================================================================
-- token_prices — USD price time series, one row per (mint, fetch)
--
-- Hypertable: pure time-series data. Compression and retention policies are
-- intentionally NOT set up here — at the v0.1 scale (handful of watched
-- pools) the volume is tiny. Policies come when the watched-pool allowlist
-- is lifted and the row count justifies them.
-- ============================================================================
CREATE TABLE token_prices (
    -- The SPL mint this price is for. No FK to token_metadata: a price
    -- may, in principle, be fetched before metadata exists.
    mint          TEXT            NOT NULL,

    -- Price in USD. NUMERIC(38, 18) covers memecoins with very small
    -- per-token values without precision loss.
    price_usd     NUMERIC(38, 18) NOT NULL,

    -- Which source produced this price: 'jupiter' | 'helius' | 'fallback'.
    price_source  TEXT            NOT NULL,

    -- Optional confidence value (Jupiter V3 does not provide one, but the
    -- column is kept for a future Helius fallback that might).
    confidence    REAL,

    -- When the price was fetched. Part of the PK and the hypertable
    -- time dimension.
    fetched_at    TIMESTAMPTZ     NOT NULL,

    PRIMARY KEY (mint, fetched_at)
);

SELECT create_hypertable(
    'token_prices',
    'fetched_at',
    chunk_time_interval => INTERVAL '7 days'
);

-- Supports "latest price for a mint" lookups (mint filter + recent ordering).
CREATE INDEX idx_token_prices_mint_recent
    ON token_prices (mint, fetched_at DESC);

GRANT SELECT, INSERT ON token_prices TO yog_context;
GRANT SELECT         ON token_prices TO yog_api;