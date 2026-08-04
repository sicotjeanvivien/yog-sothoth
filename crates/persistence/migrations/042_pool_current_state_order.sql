-- ============================================================================
-- 042 — pool_current_state stops ordering by the second
-- ============================================================================
-- The projection's upsert guard compared `last_event_at`, a TIMESTAMPTZ taken
-- from `blockTime` — so a **second**, strictly. But 56,1 % of swaps share
-- their `(pool, timestamp)` with another swap, up to 46 within one second: the
-- audit of 3 August 2026 measured **33,5 % of state updates rejected**, and
-- labelled them "stale" as though they were healthy concurrency. They were
-- not: that was the guard's own granularity.
--
-- Most visible consequence, confirmed independently while reviewing PR #96:
-- both legs of a routed transaction are persisted, but the **first** wins the
-- projection — the pool ends up showing intermediate reserves and an
-- intermediate `sqrt_price`, never the transaction's result.
--
-- These three columns carry the position of the event that produced the state.
-- The guard compares them as a tuple (see
-- `repositories/pool_current_state.rs`): `last_event_at` stays, as display
-- data, but stops being the ordering key.
--
-- ## The resulting order is partial, and measured rather than hidden
--
-- `getTransaction` does not return `transaction_index` (see migration 041), so
-- two transactions of one slot touching the same pool are ranked on
-- `event_index` alone.
--
-- ⚠️ That tie-break is not a coin flip, and calling it one would be the
-- shortcut too far: `event_index` numbers the emissions of **one** transaction,
-- so comparing it across two compares unlike things. Within a slot the state
-- converges to the largest index, which **systematically** favours a leg deep
-- inside a routed transaction over a single-leg swap of the same block. And if
-- the pool then goes quiet, the wrong state stays on display until its next
-- active slot — not for 400 ms.
--
-- What the choice buys in exchange: **independence from arrival order**. The
-- final state is a function of the set of events, not of their delivery order,
-- so a replay reproduces it. Last-writer-wins would be unbiased and
-- non-deterministic.
--
-- The guard is written with `COALESCE(last_transaction_index, 0)` so that the
-- gRPC/Geyser migration — where the transaction update carries its `index`
-- natively — makes the order total **with no migration and no code change**.
-- Until then the case is counted:
-- `yog_indexer_pool_current_state_same_slot_total`.
--
-- ## Widths follow the domain type, not the magnitude
--
-- Same rule as migration 041: `last_event_index` holds a `u16` so it is
-- INTEGER (a `u16` does not fit in a SMALLINT), `last_transaction_index` a
-- `u32` so it is BIGINT. Write conversions stay total.
--
-- ## `0` on existing rows
--
-- `DEFAULT 0` for the duration of the backfill, then dropped — a write path
-- that forgot the column must fail loudly rather than inherit a plausible `0`.
-- And `0` is not plausible: it is Solana's genesis slot (June 2020) when
-- cp-amm was deployed in 2025 and real slots sit around 300 million. An
-- impossible value, hence a sentinel: any row predating this migration is
-- superseded by the first event that arrives, which is correct — its state
-- came from the very guard being fixed here.
--
-- No renumbering, unlike migration 041: `pool_current_state` holds one row per
-- pool, nothing can collide.
--
-- ## No GRANT
--
-- `yog_indexer` already holds `INSERT, SELECT, UPDATE` at **table** level on
-- `pool_current_state` (see `tests/privileges.rs`), which covers columns added
-- later. The privilege matrix is unchanged.

ALTER TABLE pool_current_state
    ADD COLUMN last_slot              BIGINT  NOT NULL DEFAULT 0,
    ADD COLUMN last_event_index       INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN last_transaction_index BIGINT  NULL;

-- The DEFAULT only ever existed to fill the rows already in the table.
ALTER TABLE pool_current_state
    ALTER COLUMN last_slot        DROP DEFAULT,
    ALTER COLUMN last_event_index DROP DEFAULT;

-- ============================================================================
-- Dropping an index that supports nothing
-- ============================================================================
-- `idx_pool_current_state_last_event_at` has existed since migration 001 and
-- is dead weight: no query in the repository orders or range-filters on
-- `last_event_at`. The column is written, then read as a scalar of a single
-- row fetched by primary key (`PoolCurrentStateLookup::get_by_address`) — a
-- btree on it serves neither access path. Nor did it serve the old guard,
-- which compared the column on one row already located by `ON CONFLICT`.
--
-- So it was already dead before this migration; making `last_event_at` a
-- purely displayed value is simply what made us look. It cost a write on every
-- projection upsert, on the hottest write path there is, and this migration
-- makes that path fire more often — a third of the upserts it used to reject
-- now apply.
DROP INDEX idx_pool_current_state_last_event_at;
