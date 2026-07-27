-- ============================================================================
-- 030 — meteora_damm_v2_fund_reward_events (farm slot funded, emission rate set)
-- ============================================================================
-- A funder depositing reward tokens into a DAMM v2 farm slot, which makes the
-- program recompute the slot's emission rate. Decoded from the `emit_cpi!`
-- EvtFundReward (ix_fund_reward). The slot itself is opened by
-- EvtInitializeReward (migration 029) and distributes nothing until funded here.
--
-- Type mapping: u128 -> NUMERIC(39,0); u8 -> SMALLINT; u64 -> BIGINT.
--
-- ## pre_reward_rate / post_reward_rate are Q64.64 — NOT plain rates
--
-- Both are reward base units per second in Q64.64 fixed point: DIVIDE BY 2^64
-- (18446744073709551616) to read them as a rate. On a freshly opened slot,
-- verified against real on-chain data:
--
--     post_reward_rate = (amount << 64) / reward_duration
--
-- Reading these columns without the shift overstates the emission rate by 19
-- orders of magnitude. reward_duration lives on the initialize_reward row for
-- the same (pool, reward_index).
--
-- ## Carry-forward has no column, by design
--
-- Funding a slot that is still running folds the undistributed remainder of the
-- current window into the new one, so post_reward_rate reflects
-- amount + leftover. cp-amm exposes this only through the rate pair — there is
-- no carry_forward field on the event. Recover it as:
--
--     (post_reward_rate * reward_duration >> 64) - amount
--
-- amount is what the funder sent; transfer_fee_excluded_amount_in is what landed
-- in the vault (they differ only for Token-2022 mints with a transfer fee).
-- reward_duration_end is a raw unix timestamp in seconds, not a TIMESTAMPTZ —
-- same convention as initialize_pool_events.activation_point (migration 006).
--
-- Retention: NONE (kept forever). Low-frequency funder event, and the economic
-- backbone of a pool's incentive history — same treatment as migrations
-- 009 / 028 / 029. Compression still applies.

CREATE TABLE meteora_damm_v2_fund_reward_events (
    id                              BIGSERIAL,
    pool_address                    TEXT           NOT NULL,
    signature                       TEXT           NOT NULL,

    funder                          TEXT           NOT NULL,
    mint_reward                     TEXT           NOT NULL,
    reward_index                    SMALLINT       NOT NULL,
    amount                          BIGINT         NOT NULL,
    transfer_fee_excluded_amount_in BIGINT         NOT NULL,
    reward_duration_end             BIGINT         NOT NULL,
    pre_reward_rate                 NUMERIC(39, 0) NOT NULL,
    post_reward_rate                NUMERIC(39, 0) NOT NULL,

    timestamp                       TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_fund_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_fund_reward_events (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature. reward_index is
-- part of the key on purpose: one transaction can fund several slots, and a
-- (signature, timestamp)-only key would let ON CONFLICT DO NOTHING silently
-- swallow every slot after the first.
CREATE UNIQUE INDEX ON meteora_damm_v2_fund_reward_events
    (signature, reward_index, timestamp);

ALTER TABLE meteora_damm_v2_fund_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_fund_reward_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_fund_reward_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_fund_reward_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per farm funding, protocol injected.
-- Single-protocol today (DAMM v2 only); a new protocol adds a UNION ALL branch
-- selecting the same slim common columns. No reader yet (indexed trace).
-- The rate columns stay Q64.64 here too — the VIEW does not rescale.
-- ============================================================================
CREATE VIEW fund_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    funder,
    mint_reward,
    reward_index,
    amount,
    transfer_fee_excluded_amount_in,
    reward_duration_end,
    pre_reward_rate,
    post_reward_rate,
    timestamp
FROM meteora_damm_v2_fund_reward_events;

GRANT SELECT ON fund_reward_events TO yog_api;
