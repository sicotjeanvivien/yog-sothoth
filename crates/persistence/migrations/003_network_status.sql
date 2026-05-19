-- 003_network_status.sql
--
-- Network status — a SINGLE-ROW table holding the current health
-- snapshot of the indexer's link to the Solana chain.
--
-- This is deliberately a singleton, not a time series: it answers
-- "what is the state right now" for the dashboard sidebar's
-- "Solana Live" panel, nothing more. Historical health metrics
-- (latency over time, indexing gaps) are Prometheus's job, not this
-- table's.
--
-- The indexer overwrites the single row every ~15s with the latest
-- observed slot and the round-trip latency of the getSlot call.

CREATE TABLE IF NOT EXISTS network_status (
    -- Singleton guard: the CHECK constraint allows only id = 1, so
    -- the table can never hold more than one row. Writers use
    -- INSERT ... ON CONFLICT (id) DO UPDATE.
    id              SMALLINT    PRIMARY KEY DEFAULT 1
                                CHECK (id = 1),

    -- Latest Solana slot observed by the indexer.
    -- Slots are u64 on-chain; stored as BIGINT (cast at the edges).
    slot            BIGINT      NOT NULL,

    -- Round-trip latency of the getSlot RPC call, in milliseconds.
    -- A getSlot round-trip is tens-to-hundreds of ms — INTEGER is
    -- ample.
    rpc_latency_ms  INTEGER     NOT NULL,

    -- When the indexer recorded this snapshot (server-side wall
    -- clock). The API derives "data freshness" separately, from the
    -- event tables; this column is the freshness of the slot value
    -- itself.
    observed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the singleton row so the very first write is a plain UPDATE
-- path and the API never has to handle an empty table. Values are
-- placeholders, overwritten by the indexer's first tick.
INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
VALUES (1, 0, 0, now())
ON CONFLICT (id) DO NOTHING;