-- ============================================================================
-- 038 — `pools.needs_refresh` : l'invalidation remplace l'écriture indexer
-- ============================================================================
-- Splits the two roles that were tangled on the pool-property columns.
--
-- ## The problem this closes
--
-- yog-context is a **one-shot back-fill**, not a synchroniser:
-- `list_unresolved` only proposes pools with at least one NULL column, so once
-- a pool resolves it never comes back. Anything that *changes over time* was
-- therefore invisible to it, and the indexer wrote those properties itself from
-- the event stream — which is how `pools` and the DAMM v2 satellite ended up
-- with two writers each.
--
-- That second writer never worked. `yog_indexer` holds only SELECT on
-- `meteora_damm_v2_pool_properties`: migration 036 moved the fee-shape columns
-- off `pools` (where the indexer had table-level UPDATE) and granted the new
-- table to `yog_context` and `yog_api` only. Every `set_fee_config` /
-- `set_has_dynamic_fee` from the indexer has failed with `permission denied`
-- since — silently, because those writes are skip-and-log and no
-- `InitializePool` or `UpdatePoolFees` event has been observed in seven weeks.
--
-- ## The shape instead
--
-- The indexer stops writing property *values* and raises this flag. yog-context
-- re-reads the account and writes every property, remaining the single writer.
-- Reading the account also removes a whole class of decode hazard: an account
-- carries resolved state, while an update event carries a delta (see the
-- variable-offset borsh tag and the tri-state `Option` documented in the
-- `UpdatePoolFees` decoder this replaces).
--
-- ## Why a flag on `pools`, and not NULLing the column
--
-- Two reasons, one of them the bug above:
--
--   1. `pools` is where `yog_indexer` already holds UPDATE. NULLing the DAMM v2
--      satellite would need a fresh GRANT on that table — and one more on every
--      protocol's satellite as they arrive, which is exactly the step migration
--      036 forgot.
--   2. The old value stays visible while the refresh is pending, instead of the
--      dashboard showing "unknown" for a poll interval.
--
-- Protocol-neutral by construction: nothing here names a protocol, so the DLMM
-- and any later satellite reuse it as-is.
--
-- NOT NULL DEFAULT FALSE: an existing pool is not stale, it is merely already
-- resolved. Back-filling TRUE here would re-fetch all 971 pools for nothing.

ALTER TABLE pools
    ADD COLUMN needs_refresh BOOLEAN NOT NULL DEFAULT FALSE;

-- Partial index: the flag is FALSE for nearly every row nearly always, and the
-- only query that reads it wants the rare TRUE ones. A full index would be
-- mostly dead weight on a hot table.
CREATE INDEX idx_pools_needs_refresh
    ON pools (needs_refresh)
    WHERE needs_refresh;

-- The indexer raises the flag; yog-context lowers it once it has written the
-- refreshed properties.
--
-- Only one GRANT is needed, and the asymmetry is the lesson migration 036 paid
-- for: `yog_indexer` holds **table-level** UPDATE on `pools` (migration 001), so
-- a new column is covered the moment it exists. `yog_context` holds
-- **column-level** UPDATE — `(token_a_mint, token_b_mint)` from 014, `fee_bps`
-- from 015 — which by definition never extends to a column added later.
--
-- Table-level grants pick up new columns in silence; column-level ones must be
-- restated. Adding a column and forgetting this line fails at runtime, under
-- the real role only, on a write path that is skip-and-log — which is how the
-- 036 gap survived a month.
GRANT UPDATE (needs_refresh) ON pools TO yog_context;
COMMENT ON COLUMN pools.needs_refresh IS
    'Raised by the indexer when an event changes an account-derived property; '
    'cleared by yog-context after it re-reads the account. The indexer never '
    'writes property values itself — see migration 038.';
