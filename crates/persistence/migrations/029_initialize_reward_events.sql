-- ============================================================================
-- 029 — meteora_damm_v2_initialize_reward_events (farm reward slot opened)
-- ============================================================================
-- An admin opening a farming (liquidity-mining) reward slot on a DAMM v2 pool.
-- A pool carries a fixed number of slots addressed by reward_index; each streams
-- one reward_mint token to in-range LPs at a constant rate. Decoded from the
-- `emit_cpi!` EvtInitializeReward (ix_initialize_reward).
--
-- Opening a slot distributes nothing on its own: the tokens and the emission
-- rate arrive with EvtFundReward (migration 030), usually in the same
-- transaction. This row is the "a new farm launched" marker.
--
-- reward_duration: length of a funding window in SECONDS (e.g. 604800 = 7 days),
-- not a timestamp. BIGINT — same u64→i64 convention as the other event tables.
-- reward_index: on-chain u8 slot index → SMALLINT.
--
-- Retention: NONE (kept forever). Low-frequency admin event and the anchor of a
-- pool's incentive history — same treatment as migrations 009 / 028.
-- Compression still applies: it reclaims space without dropping rows.

CREATE TABLE meteora_damm_v2_initialize_reward_events (
    id              BIGSERIAL,
    pool_address    TEXT        NOT NULL,
    signature       TEXT        NOT NULL,

    reward_mint     TEXT        NOT NULL,
    funder          TEXT        NOT NULL,
    creator         TEXT        NOT NULL,
    reward_index    SMALLINT    NOT NULL,
    reward_duration BIGINT      NOT NULL,

    timestamp       TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_initialize_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_damm_v2_initialize_reward_events (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature. reward_index is
-- part of the key on purpose: a single transaction can open more than one slot,
-- and a (signature, timestamp)-only key would let ON CONFLICT DO NOTHING
-- silently swallow every slot after the first.
CREATE UNIQUE INDEX ON meteora_damm_v2_initialize_reward_events
    (signature, reward_index, timestamp);

ALTER TABLE meteora_damm_v2_initialize_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_initialize_reward_events', INTERVAL '7 days');
-- No retention policy on purpose (see header).

GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_initialize_reward_events TO yog_indexer;
GRANT SELECT                 ON meteora_damm_v2_initialize_reward_events TO yog_api;

-- ============================================================================
-- Cross-protocol VIEW — one row per reward slot opened, protocol injected.
-- Single-protocol today (DAMM v2 only); a new protocol adds a UNION ALL branch
-- selecting the same slim common columns. No reader yet (indexed trace).
-- ============================================================================
CREATE VIEW initialize_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    funder,
    creator,
    reward_index,
    reward_duration,
    timestamp
FROM meteora_damm_v2_initialize_reward_events;

GRANT SELECT ON initialize_reward_events TO yog_api;
