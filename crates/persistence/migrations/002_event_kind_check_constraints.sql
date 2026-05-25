-- ============================================================================
-- 002_event_kind_check_constraints.sql — enum-string defense in depth
-- ============================================================================
-- Adds CHECK constraints on the two enum-like string columns that the
-- application layer already validates: swap_events.trade_direction and
-- liquidity_events.liquidity_event_kind. Mirrors the existing CHECK on
-- pool_current_state.last_event_kind from the baseline schema — the goal
-- is a uniform pattern: every enum-string column carries its allowed
-- values at the schema level, so a bug, a manual INSERT, or a future
-- regression cannot silently push a value the domain code can't decode.
--
-- These constraints only reject *new* writes; existing rows are validated
-- at constraint creation time and any pre-existing violation would
-- surface as a migration failure, which is the desired behaviour
-- (operational signal rather than silent acceptance).
-- ============================================================================

ALTER TABLE swap_events
    ADD CONSTRAINT swap_events_trade_direction_valid
    CHECK (trade_direction IN ('a_to_b', 'b_to_a'));

ALTER TABLE liquidity_events
    ADD CONSTRAINT liquidity_events_kind_valid
    CHECK (liquidity_event_kind IN ('add', 'remove'));