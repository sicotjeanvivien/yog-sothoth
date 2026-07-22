-- ============================================================================
-- 027 — pools.{base_fee_kind, has_dynamic_fee} (fee shape, decoded at genesis)
-- ============================================================================
-- Companion to pools.fee_bps (migration 015): where fee_bps is the headline
-- fee *tier*, these capture how the fee *behaves*, decoded from the same raw
-- borsh PoolFeeParameters blob (meteora_damm_v2_initialize_pool_events.
-- pool_fees_raw) by core::amm::damm_v2::decode_fee_config.
--
--   base_fee_kind — how the base fee moves over time. One of:
--       'constant'              fixed fee, no scheduling
--       'scheduler_linear'      anti-sniper fee scheduler, linear decay
--       'scheduler_exponential' anti-sniper fee scheduler, exponential decay
--       'rate_limiter'          rate limiter / anti-sniper (mode 2)
--     Derived from the BaseFeeMode discriminant AND the scheduler period count:
--     a scheduler mode with zero periods is a constant fee, so the mode byte
--     alone is not enough.
--   has_dynamic_fee — whether a volatility-based dynamic fee sits on top of the
--     base fee (the Option<DynamicFeeParameters> tag is present). Orthogonal to
--     base_fee_kind: a pool can run a scheduler and a dynamic fee at once.
--
-- TEXT (not an enum type): the closed value set lives in the Rust
-- BaseFeeKind enum (the single source of truth); a Postgres enum would force a
-- second migration to extend. No CHECK for the same reason — only the indexer
-- writes this column, from that typed enum.
--
-- Both nullable: unknown between a pool's discovery (swap/liquidity stream) and
-- the arrival of its InitializePool event, and left NULL if the fee blob ever
-- fails to decode (too short / unknown mode / malformed Option tag) —
-- skip-and-log, never a wrong value.
--
-- No new GRANT: yog_indexer already holds table-level UPDATE on pools
-- (migration 001) and writes these in persist_initialize_pool, the same path
-- that already writes fee_bps; yog_api holds SELECT (default privileges).

ALTER TABLE pools
    ADD COLUMN base_fee_kind   TEXT,
    ADD COLUMN has_dynamic_fee BOOLEAN;
