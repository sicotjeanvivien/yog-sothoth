-- 002_pool_current_state.sql
--
-- Projection table holding the latest known state of each observed pool.
--
-- Maintained event-driven by the indexer: every persisted swap or liquidity
-- event triggers an upsert into this table. The table is intentionally a
-- regular relation (not a Postgres MATERIALIZED VIEW, not a Timescale
-- continuous aggregate) because we need O(1) lookups by pool_address and
-- we maintain the state ourselves from the event stream.
--
-- This is a CQRS-style read model: swap_events / liquidity_events remain the
-- source of truth (append-only), this table is an optimization for the
-- "current state of pool X" query that the dashboard needs.
--
-- Replay safety: upserts use a stale-write guard (WHERE current.last_event_at
-- < EXCLUDED.last_event_at), so reprocessing old events never overwrites a
-- newer state. See PgPoolCurrentStateRepository::upsert.

BEGIN;

-- -------------------------------------------------------------------------
-- Table
-- -------------------------------------------------------------------------
--
-- Column types intentionally mirror their counterparts in swap_events and
-- liquidity_events:
--   * reserve_a / reserve_b      BIGINT          (u64 in the domain — SPL amounts)
--   * last_sqrt_price            NUMERIC(39, 0)  (u128 Q64.64)
--   * liquidity                  NUMERIC(39, 0)  (u128 concentrated-liquidity L)

CREATE TABLE IF NOT EXISTS pool_current_state (
    pool_address       TEXT PRIMARY KEY
                       REFERENCES pools(pool_address) ON DELETE CASCADE,
    protocol           TEXT NOT NULL,

    -- Last event of any kind that touched this pool
    last_event_at      TIMESTAMPTZ NOT NULL,
    last_event_kind    TEXT NOT NULL,
    last_signature     TEXT NOT NULL,

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

-- -------------------------------------------------------------------------
-- Indexes
-- -------------------------------------------------------------------------

-- Filter the projection by protocol (multi-protocol dashboard views)
CREATE INDEX IF NOT EXISTS idx_pool_current_state_protocol
    ON pool_current_state(protocol);

-- "Most recently active pools" listing — main dashboard sort
CREATE INDEX IF NOT EXISTS idx_pool_current_state_last_event_at
    ON pool_current_state(last_event_at DESC);

-- -------------------------------------------------------------------------
-- Backfill from existing event history
-- -------------------------------------------------------------------------
--
-- For every pool already known, compute the latest state by combining:
--   * the most recent swap event (gives reserves + sqrt_price)
--   * the most recent liquidity event (gives reserves + liquidity)
-- The "winner" for the canonical (last_event_at, reserve_a, reserve_b,
-- last_signature, last_event_kind) is whichever event has the greater
-- timestamp. The sqrt_price / liquidity fields are pulled from their
-- respective sources independently because they have different domains.
--
-- This block is idempotent: ON CONFLICT DO NOTHING keeps any row that the
-- live indexer may already have written between migration apply and now.

WITH last_swap AS (
    SELECT DISTINCT ON (pool_address)
        pool_address,
        protocol,
        timestamp        AS event_at,
        signature,
        reserve_a_after  AS reserve_a,
        reserve_b_after  AS reserve_b,
        next_sqrt_price  AS sqrt_price
    FROM swap_events
    ORDER BY pool_address, timestamp DESC
),
last_liquidity AS (
    SELECT DISTINCT ON (pool_address)
        pool_address,
        protocol,
        timestamp                AS event_at,
        signature,
        liquidity_event_kind,
        reserve_a_after          AS reserve_a,
        reserve_b_after          AS reserve_b,
        liquidity_delta          AS liquidity
    FROM liquidity_events
    ORDER BY pool_address, timestamp DESC
),
combined AS (
    SELECT
        COALESCE(s.pool_address, l.pool_address) AS pool_address,
        COALESCE(s.protocol,     l.protocol)     AS protocol,

        -- Pick the most recent event between swap and liquidity
        CASE
            WHEN s.event_at IS NULL THEN l.event_at
            WHEN l.event_at IS NULL THEN s.event_at
            WHEN s.event_at >= l.event_at THEN s.event_at
            ELSE l.event_at
        END AS last_event_at,

        CASE
            WHEN s.event_at IS NULL THEN
                CASE WHEN l.liquidity_event_kind = 'add'
                     THEN 'liquidity_add' ELSE 'liquidity_remove' END
            WHEN l.event_at IS NULL THEN 'swap'
            WHEN s.event_at >= l.event_at THEN 'swap'
            ELSE
                CASE WHEN l.liquidity_event_kind = 'add'
                     THEN 'liquidity_add' ELSE 'liquidity_remove' END
        END AS last_event_kind,

        CASE
            WHEN s.event_at IS NULL THEN l.signature
            WHEN l.event_at IS NULL THEN s.signature
            WHEN s.event_at >= l.event_at THEN s.signature
            ELSE l.signature
        END AS last_signature,

        CASE
            WHEN s.event_at IS NULL THEN l.reserve_a
            WHEN l.event_at IS NULL THEN s.reserve_a
            WHEN s.event_at >= l.event_at THEN s.reserve_a
            ELSE l.reserve_a
        END AS reserve_a,

        CASE
            WHEN s.event_at IS NULL THEN l.reserve_b
            WHEN l.event_at IS NULL THEN s.reserve_b
            WHEN s.event_at >= l.event_at THEN s.reserve_b
            ELSE l.reserve_b
        END AS reserve_b,

        s.sqrt_price   AS last_sqrt_price,
        s.event_at     AS last_swap_at,
        l.liquidity    AS liquidity,
        l.event_at     AS last_liquidity_at
    FROM last_swap s
    FULL OUTER JOIN last_liquidity l USING (pool_address)
)
INSERT INTO pool_current_state (
    pool_address, protocol,
    last_event_at, last_event_kind, last_signature,
    reserve_a, reserve_b,
    last_sqrt_price, last_swap_at,
    liquidity, last_liquidity_at
)
SELECT
    pool_address, protocol,
    last_event_at, last_event_kind, last_signature,
    reserve_a, reserve_b,
    last_sqrt_price, last_swap_at,
    liquidity, last_liquidity_at
FROM combined
WHERE pool_address IN (SELECT pool_address FROM pools)
ON CONFLICT (pool_address) DO NOTHING;

COMMIT;