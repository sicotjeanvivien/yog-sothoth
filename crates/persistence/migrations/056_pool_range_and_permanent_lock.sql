-- 056_pool_range_and_permanent_lock.sql — the cp-amm Pool account's price range, its price at the
-- read, its fee collection mode and its liquidity locked for good, written by yog-context; NULL for
-- an account too short to hold them. Every DAMM v2 pool is flagged once, to be read again.

ALTER TABLE meteora_damm_v2_pool_properties
    ADD COLUMN sqrt_min_price           NUMERIC(39, 0),
    ADD COLUMN sqrt_max_price           NUMERIC(39, 0),
    ADD COLUMN sqrt_price               NUMERIC(39, 0),
    ADD COLUMN collect_fee_mode         SMALLINT,
    ADD COLUMN permanent_lock_liquidity NUMERIC(39, 0);

COMMENT ON COLUMN meteora_damm_v2_pool_properties.sqrt_price IS
    'Q64.64 square root of the price when the account was read; any later swap supersedes it.';
COMMENT ON COLUMN meteora_damm_v2_pool_properties.collect_fee_mode IS
    '0 both tokens, 1 token B only, 2 compounding: a compounding pool has no range (0, 2^128 - 1).';
COMMENT ON COLUMN meteora_damm_v2_pool_properties.permanent_lock_liquidity IS
    'Liquidity locked for good, in L, pool-wide; reread after every permanent lock.';

-- ⚠️ The flag, not the unresolved predicate: a NULL range there would requeue a short account forever.
UPDATE pools SET needs_refresh = true WHERE protocol = 'meteora_damm_v2';
