-- ============================================================================
-- 014 — token mints become a pool property (resolved by yog-context)
-- ============================================================================
-- Pool token mints were inferred per-event from the transferChecked CPIs in
-- the transaction. That heuristic is wrong on routed / multi-hop txs (a
-- Jupiter-style aggregator): the first transferChecked in the pre-event slice
-- can belong to another leg, so an ORE/USDC pool was recorded as SOL/SOL.
--
-- The authoritative source is the cp-amm Pool account (tokenAMint @ offset
-- 168, tokenBMint @ offset 200), decoded by yog-context — the same place that
-- already resolves token metadata/prices. So mints move to a pool property;
-- the denormalized per-event columns (the bug carrier) are dropped entirely,
-- including from the API contract and the continuous aggregates.

-- The CAs and cross-protocol VIEWs read the event mint columns, so drop them
-- first; recreated below without the mints.
DROP MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly;
DROP MATERIALIZED VIEW meteora_damm_v2_liquidity_events_hourly;
DROP VIEW swap_events;
DROP VIEW liquidity_events;

-- Drop the denormalized per-event mint columns.
ALTER TABLE meteora_damm_v2_swap_events      DROP COLUMN token_a_mint, DROP COLUMN token_b_mint;
ALTER TABLE meteora_damm_v2_liquidity_events DROP COLUMN token_a_mint, DROP COLUMN token_b_mint;

-- Mints are unknown at pool discovery time (resolved later from the Pool
-- account by yog-context), so they become nullable.
ALTER TABLE pools
    ALTER COLUMN token_a_mint DROP NOT NULL,
    ALTER COLUMN token_b_mint DROP NOT NULL;

-- yog-context resolves and writes the mints (column-level UPDATE only).
GRANT UPDATE (token_a_mint, token_b_mint) ON pools TO yog_context;

-- ── Cross-protocol VIEWs, rebuilt without the mint columns ──────────────────
CREATE VIEW swap_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id, pool_address, signature,
    trade_direction, amount_a, amount_b,
    reserve_a_after, reserve_b_after, timestamp
FROM meteora_damm_v2_swap_events;

CREATE VIEW liquidity_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id, pool_address, signature,
    liquidity_event_kind, amount_a, amount_b,
    reserve_a_after, reserve_b_after, position, owner, timestamp
FROM meteora_damm_v2_liquidity_events;

-- ── Volume CA (was migration 010), rebuilt without mint columns ─────────────
-- USD valuation now joins `pools` for the mints at read time.
CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                        AS bucket,
    pool_address,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_swap_events_hourly TO yog_api;

-- ── Liquidity CA (was migration 011), rebuilt without mint columns ──────────
CREATE MATERIALIZED VIEW meteora_damm_v2_liquidity_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                                   AS bucket,
    pool_address,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_a_added,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_b_added,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_a_removed,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_b_removed,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'add')    AS liquidity_added,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'remove') AS liquidity_removed,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'add')    AS add_count,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'remove') AS remove_count
FROM meteora_damm_v2_liquidity_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_liquidity_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_liquidity_events_hourly TO yog_api;
