-- ============================================================================
-- 031 — meteora_damm_v2_withdraw_ineligible_reward_events (unearnable rewards
--       returned to the funder)
-- ============================================================================
-- A funder reclaiming reward tokens that nobody could earn. Rewards accrue
-- continuously once a slot is funded (migration 030), but only in-range LPs are
-- eligible; whatever accrued while the pool held ZERO eligible liquidity can
-- never be claimed by anyone. This instruction returns it to the funder, and is
-- only permitted after the emission window has ended. Decoded from the
-- `emit_cpi!` EvtWithdrawIneligibleReward (ix_withdraw_ineligible_reward).
--
-- amount: reward base units returned. BIGINT — same u64→i64 convention as the
-- other event tables. Legitimately ZERO when the pool always had eligible
-- liquidity: the instruction still runs and still emits.
--
-- No reward_index on this event (cp-amm identifies the slot by reward_mint
-- here), so the idempotency key is the usual (signature, timestamp) — unlike
-- migrations 029/030, which needed reward_index in the key.
--
-- Note: cp-amm has a structurally identical EvtWithdrawDeadLiquidityReward
-- (same three fields) covering the reward share of permanently locked liquidity
-- with no owner to claim it. It is a DISTINCT event with its own discriminator;
-- it is not indexed yet (no fixture captured) and, when it is, it gets its own
-- table per "voie 3" rather than a kind column here.
--
-- Retention: NONE (kept forever). Low-frequency funder event completing the
-- incentive history of a pool — same treatment as migrations 009 / 028-030.

CREATE TABLE meteora_damm_v2_withdraw_ineligible_reward_events (
    id           BIGSERIAL,
    pool_address TEXT        NOT NULL,
    signature    TEXT        NOT NULL,

    reward_mint  TEXT        NOT NULL,
    amount       BIGINT      NOT NULL,

    timestamp    TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_withdraw_ineligible_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_withdraw_ineligible_reward_events (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature.
CREATE UNIQUE INDEX ON meteora_damm_v2_withdraw_ineligible_reward_events (signature, timestamp);

ALTER TABLE meteora_damm_v2_withdraw_ineligible_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_withdraw_ineligible_reward_events',
    INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_withdraw_ineligible_reward_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_withdraw_ineligible_reward_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per ineligible-reward withdrawal, protocol
-- injected. Single-protocol today (DAMM v2 only); a new protocol adds a
-- UNION ALL branch selecting the same slim common columns. No reader yet
-- (indexed trace).
-- ============================================================================
CREATE VIEW withdraw_ineligible_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    amount,
    timestamp
FROM meteora_damm_v2_withdraw_ineligible_reward_events;

GRANT SELECT ON withdraw_ineligible_reward_events TO yog_api;
