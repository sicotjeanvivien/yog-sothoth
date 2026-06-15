-- ============================================================================
-- 009 — differentiated retention: keep low-frequency events forever
-- ============================================================================
-- Retention decision (15 Jun 2026, see BACKLOG → "Stratégie de rétention &
-- historisation"): option A — only the high-volume ring-1 streams
-- (swap, liquidity) are dropped past 30d; their long-term analytics history
-- will live in the volume continuous aggregate (still to come). The punctual /
-- config events and the position-lifecycle events are low in volume but high
-- in semantic value (a pool's config lineage, the open/close/lock history of
-- each position) and must be kept indefinitely.
--
-- Migrations 001–008 applied the 7d/30d house default uniformly via the
-- add-migration template, so these tables currently carry a 30d retention
-- policy that would silently drop that history. Forward-only fix: remove only
-- the retention policy on those tables. Compression is deliberately LEFT in
-- place — it reclaims space without dropping rows and keeps chunks queryable.
--
-- NOT touched here:
--   * swap / liquidity   — retention kept (decision A). Reminder: the volume
--                          continuous aggregate must ship before any chunk
--                          crosses 30d, or that analytics history is lost.
--   * claim_position_fee / claim_reward — retention classification still open
--                          (not covered by the decision table); left as-is.

-- Position lifecycle — kept forever
SELECT remove_retention_policy('meteora_damm_v2_create_position_events',         if_exists => true);
SELECT remove_retention_policy('meteora_damm_v2_close_position_events',          if_exists => true);
SELECT remove_retention_policy('meteora_damm_v2_lock_position_events',           if_exists => true);
SELECT remove_retention_policy('meteora_damm_v2_permanent_lock_position_events', if_exists => true);

-- Pool config / admin — kept forever
SELECT remove_retention_policy('meteora_damm_v2_initialize_pool_events',         if_exists => true);
SELECT remove_retention_policy('meteora_damm_v2_set_pool_status_events',         if_exists => true);
SELECT remove_retention_policy('meteora_damm_v2_update_pool_fees_events',        if_exists => true);
