-- ============================================================================
-- 032 — meteora_damm_v2_update_reward_duration_events (farm slot re-paced)
-- ============================================================================
-- An admin changing the length of a farm slot's funding window. Decoded from the
-- `emit_cpi!` EvtUpdateRewardDuration (operator/ix_update_reward_duration.rs).
--
-- This changes the emission rate every SUBSEQUENT funding will compute
-- (rate = amount / duration, see migration 030) — it does not re-rate the window
-- already running. A duration stretched without fresh funding dilutes the farm:
-- the same tokens spread thinner, lower yield per LP.
--
-- Durations are in SECONDS (e.g. 604800 = 7 days), not timestamps.
-- reward_index: on-chain u8 slot index -> SMALLINT. cp-amm's NUM_REWARDS = 2,
-- so it is 0 or 1 in practice.
--
-- Admin-gated: the signer is the pool creator, or an operator holding the
-- UpdateRewardDuration permission.
--
-- NO ON-CHAIN FIXTURE for this event: the layout comes from the cp-amm source
-- alone (single emit_cpi! site, verified). Guarded in core by a field-mapping
-- test and a byte-level layout-pinning test rather than a real transaction.
--
-- Retention: NONE (kept forever). Low-frequency admin event belonging to a
-- pool's incentive history — same treatment as migrations 009 / 028-031.

CREATE TABLE meteora_damm_v2_update_reward_duration_events (
    id                  BIGSERIAL,
    pool_address        TEXT        NOT NULL,
    signature           TEXT        NOT NULL,

    reward_index        SMALLINT    NOT NULL,
    old_reward_duration BIGINT      NOT NULL,
    new_reward_duration BIGINT      NOT NULL,

    timestamp           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_reward_duration_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_update_reward_duration_events (pool_address, timestamp DESC);
-- Idempotency guard. reward_index is part of the key: one transaction can
-- re-pace more than one slot, and a (signature, timestamp)-only key would let
-- ON CONFLICT DO NOTHING silently swallow every slot after the first.
CREATE UNIQUE INDEX ON meteora_damm_v2_update_reward_duration_events
    (signature, reward_index, timestamp);

ALTER TABLE meteora_damm_v2_update_reward_duration_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_reward_duration_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_update_reward_duration_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_update_reward_duration_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per re-pacing, protocol injected.
-- Single-protocol today (DAMM v2 only); a new protocol adds a UNION ALL branch
-- selecting the same slim common columns. No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW update_reward_duration_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_index,
    old_reward_duration,
    new_reward_duration,
    timestamp
FROM meteora_damm_v2_update_reward_duration_events;

GRANT SELECT ON update_reward_duration_events TO yog_api;
