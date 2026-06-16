-- ============================================================================
-- 016 — yog_context may write pools.fee_bps
-- ============================================================================
-- yog-context now resolves the base fee from the on-chain Pool account (the
-- same fetch that resolves the mints) and writes it via set_pool_account.
-- Migration 014 granted column-level UPDATE on the mint columns only; 015
-- added fee_bps but granted nothing to yog_context (the indexer wrote it then).
-- Without this grant, set_pool_account fails with "permission denied for table
-- pools" — the least-privilege model working as designed.

GRANT UPDATE (fee_bps) ON pools TO yog_context;
