-- ============================================================================
-- 033 — meteora_damm_v2_update_reward_funder_events (farm funding right moved)
-- ============================================================================
-- An admin transferring the right to fund a farm slot from one wallet to
-- another. Decoded from the `emit_cpi!` EvtUpdateRewardFunder
-- (operator/ix_update_reward_funder.rs).
--
-- Moves no tokens and does not touch the emission rate: it only changes which
-- wallet may call fund_reward on this reward_index, and which wallet receives
-- reclaimed rewards. Read as provenance — who is paying for the incentive, and
-- when the farm changed hands.
--
-- reward_index: on-chain u8 slot index -> SMALLINT (NUM_REWARDS = 2 in cp-amm).
--
-- Admin-gated: the signer is the pool creator, or an operator holding the
-- UpdateRewardFunder permission.
--
-- NO ON-CHAIN FIXTURE for this event: the layout comes from the cp-amm source
-- alone (single emit_cpi! site, verified). Guarded in core by a field-mapping
-- test and a byte-level layout-pinning test rather than a real transaction.
--
-- Retention: NONE (kept forever). Same treatment as migrations 009 / 028-032.

CREATE TABLE meteora_damm_v2_update_reward_funder_events (
    id           BIGSERIAL,
    pool_address TEXT        NOT NULL,
    signature    TEXT        NOT NULL,

    reward_index SMALLINT    NOT NULL,
    old_funder   TEXT        NOT NULL,
    new_funder   TEXT        NOT NULL,

    timestamp    TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_reward_funder_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_update_reward_funder_events (pool_address, timestamp DESC);
-- Idempotency guard, reward_index in the key for the same multi-slot reason as
-- migrations 029 / 030 / 032.
CREATE UNIQUE INDEX ON meteora_damm_v2_update_reward_funder_events
    (signature, reward_index, timestamp);

ALTER TABLE meteora_damm_v2_update_reward_funder_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_reward_funder_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_update_reward_funder_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_update_reward_funder_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per funder hand-over, protocol injected.
-- Single-protocol today (DAMM v2 only); a new protocol adds a UNION ALL branch
-- selecting the same slim common columns. No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW update_reward_funder_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_index,
    old_funder,
    new_funder,
    timestamp
FROM meteora_damm_v2_update_reward_funder_events;

GRANT SELECT ON update_reward_funder_events TO yog_api;
