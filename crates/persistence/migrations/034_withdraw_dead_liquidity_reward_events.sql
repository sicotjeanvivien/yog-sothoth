-- ============================================================================
-- 034 — meteora_damm_v2_withdraw_dead_liquidity_reward_events
-- ============================================================================
-- A funder reclaiming the reward share that accrued to DEAD LIQUIDITY —
-- liquidity permanently locked with no owner left to claim against it. Rewards
-- still accrue to it and nobody can ever collect them; this returns that share
-- to the funder. Decoded from the `emit_cpi!` EvtWithdrawDeadLiquidityReward
-- (ix_withdraw_dead_liquidity_reward.rs).
--
-- ## Distinct from migration 031, despite an identical shape
--
-- meteora_damm_v2_withdraw_ineligible_reward_events (031) has the SAME three
-- columns and the same 72-byte wire payload; only the Anchor discriminator tells
-- the two events apart. They stay separate tables because they record different
-- on-chain facts (per "voie 3": one on-chain event -> one table), and because
-- their emission semantics differ:
--
--   * 031 (ineligible) emits UNCONDITIONALLY — amount = 0 rows exist and are
--     legitimate (the captured fixture is exactly that case).
--   * this table emits only inside `if dead_liquidity_reward > 0`, so amount is
--     ALWAYS > 0 here. A zero row would mean our decoding drifted.
--
-- No reward_index on this event (cp-amm identifies the slot by reward_mint),
-- so the idempotency key is the plain (signature, timestamp) — same as 031,
-- unlike 029/030/032/033.
--
-- NO ON-CHAIN FIXTURE for this event: the layout comes from the cp-amm source
-- alone (single emit_cpi! site, verified). Guarded in core by a field-mapping
-- test and a byte-level layout-pinning test rather than a real transaction.
--
-- Retention: NONE (kept forever). Same treatment as migrations 009 / 028-033.

CREATE TABLE meteora_damm_v2_withdraw_dead_liquidity_reward_events (
    id           BIGSERIAL,
    pool_address TEXT        NOT NULL,
    signature    TEXT        NOT NULL,

    reward_mint  TEXT        NOT NULL,
    amount       BIGINT      NOT NULL,

    timestamp    TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_withdraw_dead_liquidity_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_withdraw_dead_liquidity_reward_events
    (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature.
CREATE UNIQUE INDEX ON meteora_damm_v2_withdraw_dead_liquidity_reward_events
    (signature, timestamp);

ALTER TABLE meteora_damm_v2_withdraw_dead_liquidity_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_withdraw_dead_liquidity_reward_events',
    INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_withdraw_dead_liquidity_reward_events
    TO yog_indexer;
GRANT SELECT ON meteora_damm_v2_withdraw_dead_liquidity_reward_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per dead-liquidity reward withdrawal, protocol
-- injected. Deliberately NOT unioned with withdraw_ineligible_reward_events:
-- the two describe different facts and a merged VIEW would erase that.
-- No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW withdraw_dead_liquidity_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    amount,
    timestamp
FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events;

GRANT SELECT ON withdraw_dead_liquidity_reward_events TO yog_api;
