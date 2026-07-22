-- ============================================================================
-- 028 — meteora_damm_v2_claim_protocol_fee_events (operator protocol-fee claim)
-- ============================================================================
-- The protocol operator withdrawing Meteora's accrued *protocol* share of a
-- pool's trading fees — distinct from meteora_damm_v2_claim_position_fee_events
-- (an LP claiming their own position's fees). Decoded from the `emit_cpi!`
-- EvtClaimProtocolFee (ix_claim_protocol_fee); the differently-shaped
-- EvtClaimProtocolFee2 (ix_claim_protocol_fee2) is a plain `emit!` log, not an
-- event_cpi, and is not captured by the indexer.
--
-- token_a_amount / token_b_amount: absolute amounts withdrawn this claim,
-- canonical pool ordering. BIGINT — same u64→i64 convention as the other fee
-- events (values are token base units, never negative).
--
-- Retention: NONE (kept forever). This is a low-frequency operator event and a
-- protocol-revenue trace worth keeping long-term — the same treatment migration
-- 009 gave the other low-frequency / admin events (create/close/lock/
-- initialize/update_fees). Compression is still applied: it reclaims space
-- without dropping rows and keeps chunks queryable.

CREATE TABLE meteora_damm_v2_claim_protocol_fee_events (
    id             BIGSERIAL,
    pool_address   TEXT        NOT NULL,
    signature      TEXT        NOT NULL,

    token_a_amount BIGINT      NOT NULL,
    token_b_amount BIGINT      NOT NULL,

    timestamp      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_claim_protocol_fee_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_claim_protocol_fee_events (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature.
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_protocol_fee_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_claim_protocol_fee_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_claim_protocol_fee_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_claim_protocol_fee_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_claim_protocol_fee_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per protocol-fee claim, protocol injected.
-- Single-protocol today (DAMM v2 only); a new protocol adds a UNION ALL branch
-- selecting the same slim common columns. No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW claim_protocol_fee_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    token_a_amount,
    token_b_amount,
    timestamp
FROM meteora_damm_v2_claim_protocol_fee_events;

GRANT SELECT ON claim_protocol_fee_events TO yog_api;
