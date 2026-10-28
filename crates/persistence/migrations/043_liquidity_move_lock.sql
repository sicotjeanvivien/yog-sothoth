-- 043_liquidity_move_lock.sql — each liquidity move says the lock its transaction puts on a
-- position: permanent, vesting or none. No backfill: a move valued before this migration keeps
-- `lock` NULL, not measured.

ALTER TABLE analytic_liquidity_moves
    ADD COLUMN lock TEXT CHECK (lock IN ('vesting', 'permanent', 'none'));

GRANT SELECT ON meteora_damm_v2_lock_position_events, meteora_damm_v2_permanent_lock_position_events
    TO yog_analytic;
