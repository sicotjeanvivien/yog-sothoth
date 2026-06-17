-- ============================================================================
-- 018 — pools.{protocol,partner,referral}_fee_percent (fee split config)
-- ============================================================================
-- The cp-amm `Pool` account splits each swap's trading fee between the LPs and
-- three payees, as integer percentages in PoolFeesStruct (right after the
-- base_fee blob whose cliff numerator already feeds pools.fee_bps):
--   - protocol_fee_percent — Meteora's cut of the trading fee
--   - partner_fee_percent  — a partner's cut (often 0)
--   - referral_fee_percent — a referrer's cut (only charged when the swap
--                            carries a referral account)
-- yog-context decodes them from the same on-chain account fetch that resolves
-- the mints and fee_bps (PoolAccountWorker), at byte offsets 48/49/50 —
-- empirically verified against mainnet.
--
-- SMALLINT: the on-chain values are u8 (0..=100); Postgres has no u8, and
-- SMALLINT is the smallest signed integer that holds the range losslessly.
-- Nullable: unknown between a pool's discovery and the worker resolving its
-- account; left NULL if the account ever fails to decode (skip-and-log).
--
-- GRANT UPDATE to yog_context only — it writes these via set_pool_account, the
-- same path that already writes mints (migration 014) and fee_bps (016). yog_api
-- keeps SELECT (default privileges). Without the grant set_pool_account would
-- fail "permission denied for table pools" — the least-privilege model working.

ALTER TABLE pools
    ADD COLUMN protocol_fee_percent SMALLINT,
    ADD COLUMN partner_fee_percent  SMALLINT,
    ADD COLUMN referral_fee_percent SMALLINT;

GRANT UPDATE (protocol_fee_percent, partner_fee_percent, referral_fee_percent)
    ON pools TO yog_context;
