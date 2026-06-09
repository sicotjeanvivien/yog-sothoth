-- ============================================================================
-- 001_initial_schema.sql — yog-sothoth baseline schema (v0.1)
-- ============================================================================
-- Consolidated baseline — fuses the historical migrations 001-004 and
-- reshapes the event tables per the per-protocol design (one table per
-- (protocol, event_kind), with cross-protocol VIEWs at the bottom for
-- unified reads).
--
-- Conventions:
--   - Forward-only migrations from this baseline onwards.
--   - Each migration that creates a table emits its GRANT statements at
--     the end of the relevant section. setup_roles.sql only declares
--     roles and their structural privileges; table-specific grants live
--     next to the table they apply to.
--   - The v0.1 baseline supports only Meteora DAMM v2. Future protocols
--     (DLMM, Raydium CLMM, Orca Whirlpool, …) will arrive as additional
--     sibling tables (meteora_dlmm_swap_events, raydium_clmm_swap_events,
--     …) and as additional UNION ALL branches in the cross-protocol
--     VIEWs at the bottom of this file.
--
-- watched_pools (Phase 1 temporary constraint):
--   Until the indexer runs on an upgraded RPC path (Helius
--   transactionSubscribe or equivalent), ingestion is bounded to an
--   allowlist of pools stored in `watched_pools`. The day we lift the
--   allowlist, the table stays — it just stops being read by the
--   indexer's startup filter.
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;


-- ============================================================================
-- pools — generic registry of pools discovered from the transaction stream
--
-- Cross-protocol table: `protocol` is meaningful here and is part of the
-- row identity. A pool address is unique across protocols by Solana
-- design (PDA of program + seeds), so pool_address remains the PK.
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
-- pool_current_state — CQRS read model: latest known state of each pool
--
-- Cross-protocol (one row per pool, regardless of protocol). Maintained
-- event-driven by the indexer: every persisted swap or liquidity event
-- triggers an upsert into this table. Replay safety: upserts use a
-- stale-write guard so reprocessing old events never overwrites a newer
-- state (see PgPoolCurrentStateRepository::upsert).
-- ============================================================================
CREATE TABLE pool_current_state (
    pool_address       TEXT PRIMARY KEY
                       REFERENCES pools(pool_address) ON DELETE CASCADE,
    protocol           TEXT NOT NULL,

    -- Last event of any kind that touched this pool
    last_event_at      TIMESTAMPTZ NOT NULL,
    last_event_kind    TEXT        NOT NULL,
    last_signature     TEXT        NOT NULL,

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
-- ============================================================================
CREATE TABLE network_status (
    -- Singleton guard: CHECK (id = 1) allows only one row.
    id              SMALLINT    PRIMARY KEY DEFAULT 1
                                CHECK (id = 1),

    slot            BIGINT      NOT NULL,
    rpc_latency_ms  INTEGER     NOT NULL,
    observed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed the singleton row so the very first indexer write is a plain
-- UPDATE path and the API never has to handle an empty table.
INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
VALUES (1, 0, 0, NOW());

GRANT SELECT, INSERT, UPDATE ON network_status TO yog_indexer;
GRANT SELECT                 ON network_status TO yog_api;


-- ============================================================================
-- meteora_damm_v2_swap_events — DAMM v2 swap events (Anchor EvtSwap2)
--
-- All amount and reserve fields follow the canonical (token_a, token_b)
-- pool ordering. The trader's perspective is recovered by combining
-- `trade_direction` with `amount_a` / `amount_b`.
-- ============================================================================
CREATE TABLE meteora_damm_v2_swap_events (
    id                 BIGSERIAL,
    pool_address       TEXT           NOT NULL,
    signature          TEXT           NOT NULL,

    -- Pool tokens (canonical order)
    token_a_mint       TEXT           NOT NULL,
    token_b_mint       TEXT           NOT NULL,

    -- Direction and amounts
    trade_direction    TEXT           NOT NULL,
    amount_a           BIGINT         NOT NULL,
    amount_b           BIGINT         NOT NULL,

    -- Post-swap pool state
    reserve_a_after    BIGINT         NOT NULL,
    reserve_b_after    BIGINT         NOT NULL,
    next_sqrt_price    NUMERIC(39, 0) NOT NULL,

    -- Fee breakdown (DAMM v2 specific)
    claiming_fee       BIGINT         NOT NULL,
    protocol_fee       BIGINT         NOT NULL,
    compounding_fee    BIGINT         NOT NULL,
    referral_fee       BIGINT         NOT NULL,
    fee_token_is_a     BOOLEAN        NOT NULL,

    timestamp          TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp),

    CONSTRAINT meteora_damm_v2_swap_events_trade_direction_valid
        CHECK (trade_direction IN ('a_to_b', 'b_to_a'))
);


-- ============================================================================
-- meteora_damm_v2_liquidity_events — DAMM v2 add/remove liquidity
-- ============================================================================
CREATE TABLE meteora_damm_v2_liquidity_events (
    id                   BIGSERIAL,
    pool_address         TEXT           NOT NULL,
    signature            TEXT           NOT NULL,

    token_a_mint         TEXT           NOT NULL,
    token_b_mint         TEXT           NOT NULL,

    liquidity_event_kind TEXT           NOT NULL,
    amount_a             BIGINT         NOT NULL,
    amount_b             BIGINT         NOT NULL,
    liquidity_delta      NUMERIC(39, 0) NOT NULL,

    reserve_a_after      BIGINT         NOT NULL,
    reserve_b_after      BIGINT         NOT NULL,

    position             TEXT           NOT NULL,
    owner                TEXT           NOT NULL,

    timestamp            TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp),

    CONSTRAINT meteora_damm_v2_liquidity_events_kind_valid
        CHECK (liquidity_event_kind IN ('add', 'remove'))
);


-- ============================================================================
-- meteora_damm_v2_claim_position_fee_events — LP claim of position fees
-- ============================================================================
CREATE TABLE meteora_damm_v2_claim_position_fee_events (
    id             BIGSERIAL,
    pool_address   TEXT        NOT NULL,
    signature      TEXT        NOT NULL,

    position       TEXT        NOT NULL,
    owner          TEXT        NOT NULL,

    fee_a_claimed  BIGINT      NOT NULL,
    fee_b_claimed  BIGINT      NOT NULL,

    timestamp      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);


-- ============================================================================
-- meteora_damm_v2_claim_reward_events — LP claim of farming rewards
-- ============================================================================
CREATE TABLE meteora_damm_v2_claim_reward_events (
    id            BIGSERIAL,
    pool_address  TEXT        NOT NULL,
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
-- Hypertables — DAMM v2 event tables become time-partitioned
-- ============================================================================
SELECT create_hypertable('meteora_damm_v2_swap_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('meteora_damm_v2_liquidity_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('meteora_damm_v2_claim_position_fee_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
SELECT create_hypertable('meteora_damm_v2_claim_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');


-- ============================================================================
-- Indexes for common query patterns
-- ============================================================================
CREATE INDEX ON meteora_damm_v2_swap_events               (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_liquidity_events          (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_claim_position_fee_events (pool_address, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_claim_reward_events       (pool_address, timestamp DESC);

-- Per-position queries: useful for LP dashboards.
CREATE INDEX ON meteora_damm_v2_liquidity_events          (position, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_claim_position_fee_events (position, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_claim_reward_events       (position, timestamp DESC);

-- Idempotency: replay-safe inserts via ON CONFLICT DO NOTHING.
CREATE UNIQUE INDEX ON meteora_damm_v2_swap_events               (signature, timestamp);
CREATE UNIQUE INDEX ON meteora_damm_v2_liquidity_events          (signature, timestamp);
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_position_fee_events (signature, timestamp);
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_reward_events       (signature, timestamp);


-- ============================================================================
-- Compression (after 7 days)
-- ============================================================================
ALTER TABLE meteora_damm_v2_swap_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
ALTER TABLE meteora_damm_v2_liquidity_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
ALTER TABLE meteora_damm_v2_claim_position_fee_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
ALTER TABLE meteora_damm_v2_claim_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);

SELECT add_compression_policy('meteora_damm_v2_swap_events',               INTERVAL '7 days');
SELECT add_compression_policy('meteora_damm_v2_liquidity_events',          INTERVAL '7 days');
SELECT add_compression_policy('meteora_damm_v2_claim_position_fee_events', INTERVAL '7 days');
SELECT add_compression_policy('meteora_damm_v2_claim_reward_events',       INTERVAL '7 days');


-- ============================================================================
-- Retention (30 days for raw data, Phase 1 / MVP)
-- ============================================================================
SELECT add_retention_policy('meteora_damm_v2_swap_events',               INTERVAL '30 days');
SELECT add_retention_policy('meteora_damm_v2_liquidity_events',          INTERVAL '30 days');
SELECT add_retention_policy('meteora_damm_v2_claim_position_fee_events', INTERVAL '30 days');
SELECT add_retention_policy('meteora_damm_v2_claim_reward_events',       INTERVAL '30 days');


-- ============================================================================
-- Grants on the DAMM v2 event tables
-- ============================================================================
GRANT SELECT, INSERT, UPDATE
    ON meteora_damm_v2_swap_events,
       meteora_damm_v2_liquidity_events,
       meteora_damm_v2_claim_position_fee_events,
       meteora_damm_v2_claim_reward_events
    TO yog_indexer;

GRANT SELECT
    ON meteora_damm_v2_swap_events,
       meteora_damm_v2_liquidity_events,
       meteora_damm_v2_claim_position_fee_events,
       meteora_damm_v2_claim_reward_events
    TO yog_api;

-- Sequences behind BIGSERIAL columns.
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_indexer;


-- ============================================================================
-- token_metadata — one row per mint, near-immutable reference data
-- (populated by yog-context's metadata worker via Helius DAS)
-- ============================================================================
CREATE TABLE token_metadata (
    mint              TEXT        PRIMARY KEY,
    symbol            TEXT,
    name              TEXT,
    decimals          SMALLINT    NOT NULL,
    logo_uri          TEXT,
    metadata_provider TEXT        NOT NULL DEFAULT 'helius_das',
    fetched_at        TIMESTAMPTZ NOT NULL,
    last_refresh_at   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_token_metadata_last_refresh
    ON token_metadata (last_refresh_at);

GRANT SELECT, INSERT, UPDATE ON token_metadata TO yog_context;
GRANT SELECT                 ON token_metadata TO yog_api;


-- ============================================================================
-- token_prices — USD price time series, one row per (mint, fetch)
-- (populated by yog-context's price worker via Jupiter V3)
-- ============================================================================
CREATE TABLE token_prices (
    mint           TEXT            NOT NULL,
    price_usd      NUMERIC(38, 18) NOT NULL,
    price_provider TEXT            NOT NULL,
    confidence     REAL,
    fetched_at     TIMESTAMPTZ     NOT NULL,
    PRIMARY KEY (mint, fetched_at)
);

SELECT create_hypertable(
    'token_prices',
    'fetched_at',
    chunk_time_interval => INTERVAL '7 days'
);

CREATE INDEX idx_token_prices_mint_recent
    ON token_prices (mint, fetched_at DESC);

GRANT SELECT, INSERT ON token_prices TO yog_context;
GRANT SELECT         ON token_prices TO yog_api;


-- ============================================================================
-- Cross-protocol VIEWs — unified read surface
--
-- These VIEWs expose the slim common columns across protocols, with a
-- `protocol` text column injected per UNION ALL branch. Today there is
-- only one underlying table per VIEW (meteora_damm_v2_*); future
-- protocols add UNION ALL branches without touching the API code.
--
-- Protocol-specific columns (next_sqrt_price, fee breakdown, etc.) are
-- NOT in the VIEW — code that needs them reads the underlying table
-- directly.
-- ============================================================================

CREATE VIEW swap_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    token_a_mint,
    token_b_mint,
    trade_direction,
    amount_a,
    amount_b,
    reserve_a_after,
    reserve_b_after,
    timestamp
FROM meteora_damm_v2_swap_events;

CREATE VIEW liquidity_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    token_a_mint,
    token_b_mint,
    liquidity_event_kind,
    amount_a,
    amount_b,
    reserve_a_after,
    reserve_b_after,
    position,
    owner,
    timestamp
FROM meteora_damm_v2_liquidity_events;

CREATE VIEW claim_position_fee_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    position,
    owner,
    fee_a_claimed,
    fee_b_claimed,
    timestamp
FROM meteora_damm_v2_claim_position_fee_events;

CREATE VIEW claim_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    position,
    owner,
    mint_reward,
    reward_index,
    total_reward,
    timestamp
FROM meteora_damm_v2_claim_reward_events;

GRANT SELECT ON swap_events, liquidity_events,
                claim_position_fee_events, claim_reward_events
            TO yog_api;