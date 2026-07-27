-- ============================================================================
-- 035 — meteora_damm_v2_split_position_events (position contents split toward
--       another position, possibly another wallet)
-- ============================================================================
-- A position transfers a FRACTION of its contents into a second position, which
-- may belong to a different owner. Each component is split independently:
-- unlocked liquidity, permanently locked liquidity, vesting liquidity, pending
-- fees A/B, pending farm rewards 0/1.
--
-- Product angle: a split moves liquidity BETWEEN TWO WALLETS and leaves a
-- traceable event — unlike transferring the position NFT outright, which is the
-- blind spot of any LP-concentration score. Splits are therefore visible to
-- concentration analytics (see score-pool-a-concentration-lp).
--
-- ## Source event: EvtSplitPosition3 only
--
-- cp-amm has two instructions (`split_position`, `split_position2`) routing to
-- one handler, which emits BOTH `EvtSplitPosition2` (deprecated since 0.1.8) and
-- `EvtSplitPosition3` unconditionally on every split. They describe the SAME
-- split; v3 is a strict superset (v2 collapses the three liquidity buckets into
-- a single total, and has neither vested_liquidity nor the inner-vesting
-- numerator). The indexer therefore recognises the v2 discriminator and drops it
-- deliberately — indexing both would double-count every split.
--
-- Note there is no `split_position3` instruction and no `EvtSplitPosition` v1:
-- cp-amm versions events and instructions independently and the numbers never
-- line up.
--
-- ## Column groups
--
--   split_*   what actually MOVED from the first position to the second
--   first_*   state of the first position AFTER the split
--   second_*  state of the second position AFTER the split
--   num_*     the fractions REQUESTED, numerators over 1e9
--             (SPLIT_POSITION_DENOMINATOR). Kept alongside the realised amounts
--             because the gap between the two is itself informative: rounding,
--             or a component that had nothing to give.
--
-- Type mapping: u128 -> NUMERIC(39,0); u64 -> BIGINT; u32 -> BIGINT (lossless;
-- u32 overflows INTEGER even though the on-chain validation caps numerators at
-- 1e9).
--
-- Retention: NONE (kept forever). Low-frequency event, and an ownership-transfer
-- trace whose whole value is historical — same treatment as migrations
-- 009 / 028-034.

CREATE TABLE meteora_damm_v2_split_position_events (
    id                                BIGSERIAL,
    pool_address                      TEXT           NOT NULL,
    signature                         TEXT           NOT NULL,

    first_owner                       TEXT           NOT NULL,
    second_owner                      TEXT           NOT NULL,
    first_position                    TEXT           NOT NULL,
    second_position                   TEXT           NOT NULL,
    current_sqrt_price                NUMERIC(39, 0) NOT NULL,

    -- what moved
    split_permanent_locked_liquidity  NUMERIC(39, 0) NOT NULL,
    split_unlocked_liquidity          NUMERIC(39, 0) NOT NULL,
    split_vested_liquidity            NUMERIC(39, 0) NOT NULL,
    split_fee_a                       BIGINT         NOT NULL,
    split_fee_b                       BIGINT         NOT NULL,
    split_reward_0                    BIGINT         NOT NULL,
    split_reward_1                    BIGINT         NOT NULL,

    -- first position, after
    first_unlocked_liquidity          NUMERIC(39, 0) NOT NULL,
    first_permanent_locked_liquidity  NUMERIC(39, 0) NOT NULL,
    first_vested_liquidity            NUMERIC(39, 0) NOT NULL,
    first_fee_a                       BIGINT         NOT NULL,
    first_fee_b                       BIGINT         NOT NULL,
    first_reward_0                    BIGINT         NOT NULL,
    first_reward_1                    BIGINT         NOT NULL,

    -- second position, after
    second_unlocked_liquidity         NUMERIC(39, 0) NOT NULL,
    second_permanent_locked_liquidity NUMERIC(39, 0) NOT NULL,
    second_vested_liquidity           NUMERIC(39, 0) NOT NULL,
    second_fee_a                      BIGINT         NOT NULL,
    second_fee_b                      BIGINT         NOT NULL,
    second_reward_0                   BIGINT         NOT NULL,
    second_reward_1                   BIGINT         NOT NULL,

    -- requested fractions, numerators over 1e9
    num_unlocked_liquidity            BIGINT         NOT NULL,
    num_permanent_locked_liquidity    BIGINT         NOT NULL,
    num_fee_a                         BIGINT         NOT NULL,
    num_fee_b                         BIGINT         NOT NULL,
    num_reward_0                      BIGINT         NOT NULL,
    num_reward_1                      BIGINT         NOT NULL,
    num_inner_vesting_liquidity       BIGINT         NOT NULL,

    timestamp                         TIMESTAMPTZ    NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_split_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_split_position_events (pool_address, timestamp DESC);
-- Owner-side lookups are the concentration use case: "what left / reached this
-- wallet". Two indexes because a split has two sides and either can be the
-- subject of the question.
CREATE INDEX ON meteora_damm_v2_split_position_events (first_owner, timestamp DESC);
CREATE INDEX ON meteora_damm_v2_split_position_events (second_owner, timestamp DESC);
-- Idempotency guard. second_position is part of the key: one transaction can
-- split the same source position toward several targets, and a
-- (signature, timestamp)-only key would let ON CONFLICT DO NOTHING swallow
-- every split after the first.
CREATE UNIQUE INDEX ON meteora_damm_v2_split_position_events
    (signature, second_position, timestamp);

ALTER TABLE meteora_damm_v2_split_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_split_position_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_split_position_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_split_position_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per split, protocol injected. Slim on purpose:
-- the identity of the transfer (who → who, which positions, how much liquidity
-- moved) is the cross-protocol concept. The post-split position states and the
-- requested numerators stay protocol-specific — read the table for those.
-- No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW split_position_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    first_owner,
    second_owner,
    first_position,
    second_position,
    split_permanent_locked_liquidity,
    split_unlocked_liquidity,
    split_vested_liquidity,
    timestamp
FROM meteora_damm_v2_split_position_events;

GRANT SELECT ON split_position_events TO yog_api;
