-- ============================================================================
-- 022 — signals
-- ============================================================================
-- Signal Engine (v0.1.1). A signal is a *conclusion* — a uniform shape across
-- protocols — NOT a raw on-chain event. It therefore does NOT follow the
-- per-protocol "voie 3" table split; it joins the generic single-table family
-- (pools / token_prices): ONE table, `protocol` is a column, discriminated
-- additionally by `detector` (the rule that fired).
--
-- Level-1 schema (decided 1 July 2026): typed common columns only, NO JSONB.
-- If a detector ever needs a structured payload it escalates via a nullable
-- `details JSONB` (level 2) or an extension table joined by id (level 3) —
-- never before a real detector proves the need.
--
-- Hypertable on `triggered_at` (time-series, like token_prices). Signals are
-- the product's valuable, sparse output: NO retention policy (kept
-- indefinitely); compression only kicks in after 30d.
-- ============================================================================

CREATE TABLE signals (
    id            BIGSERIAL,
    detector      TEXT        NOT NULL,   -- e.g. 'flow_imbalance', 'price_oracle_deviation'
    protocol      TEXT        NOT NULL,   -- e.g. 'meteora_damm_v2'
    pool_address  TEXT        NOT NULL,

    severity      TEXT        NOT NULL,
    value         NUMERIC     NOT NULL,   -- the metric that crossed the threshold
    threshold     NUMERIC,                -- the threshold crossed (traceability; nullable)
    message       TEXT,                   -- optional human-readable summary

    triggered_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, triggered_at),       -- triggered_at must be in the PK (hypertable)

    -- DB-level safety net, same spirit as swap_events' trade_direction CHECK.
    CONSTRAINT signals_severity_valid
        CHECK (severity IN ('info', 'warning', 'critical'))
);

SELECT create_hypertable('signals',
    'triggered_at', chunk_time_interval => INTERVAL '7 days');

-- Per-pool feed (pool detail page) and per-detector feed (dashboard filter).
CREATE INDEX ON signals (pool_address, triggered_at DESC);
CREATE INDEX ON signals (detector,     triggered_at DESC);

ALTER TABLE signals SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'triggered_at DESC',
    timescaledb.compress_segmentby = 'detector'
);
SELECT add_compression_policy('signals', INTERVAL '30 days');
-- No add_retention_policy: signals are sparse and are the product output — keep them.

-- ---------------------------------------------------------------------------
-- GRANTs — the signal-engine (yog_signals) writes; the api reads for the feed.
-- Signals are append-only conclusions: INSERT, never UPDATE. SELECT is granted
-- to yog_signals too so a detector can dedup against its own recent emissions.
-- ---------------------------------------------------------------------------
GRANT SELECT, INSERT ON signals                 TO yog_signals;
GRANT USAGE, SELECT  ON SEQUENCE signals_id_seq TO yog_signals;
GRANT SELECT         ON signals                 TO yog_api;
