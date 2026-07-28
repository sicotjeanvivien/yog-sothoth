-- ============================================================================
-- 036 — meteora_damm_v2_pool_properties (sortie des propriétés cp-amm de pools)
-- ============================================================================
-- Moves the five cp-amm-shaped property columns out of the cross-protocol
-- `pools` registry into a per-protocol satellite table.
--
-- ## Why
--
-- Migration 001 defined `pools` correctly: an address, a protocol, a token
-- pair, a first/last sighting — identity and discovery metadata, genuinely
-- identical for every protocol. Five property columns then accreted on top:
--
--   protocol_fee_percent, partner_fee_percent, referral_fee_percent  (018)
--   base_fee_kind, has_dynamic_fee                                   (027)
--
-- All five are cp-amm concepts. Each was justified locally when added, and the
-- single-protocol world hid the problem. "voie 3" forbids NULL columns for
-- incompatible fields on event tables; the rule was never applied to `pools`.
-- With DLMM arriving, these five would be NULL for an entire protocol — which
-- is what makes the drift visible rather than new.
--
-- ## What stays
--
-- `fee_bps` (migration 015) stays in `pools` deliberately. Unlike the five, it
-- is a normalized cross-protocol notion AND a read surface: it is filtered
-- (`WHERE fee_bps = $n`, the pool-list fee-tier filter) and aggregated
-- (`list_fee_tiers`, `GROUP BY fee_bps`). DLMM has an effective base fee in bps
-- too. The five moved columns are SELECT-only by comparison — never filtered,
-- never ordered on.
--
-- ## Ordering is load-bearing
--
-- Backfill BEFORE the DROP, in this one file. Migrations are forward-only:
-- there is no second chance at the data once the columns are gone.
--
-- Column types are carried over unchanged from 018/027, so the backfill is a
-- straight copy. Semantics of each column are documented in those migrations
-- and are not restated here.

CREATE TABLE meteora_damm_v2_pool_properties (
    pool_address         TEXT     PRIMARY KEY
                                  REFERENCES pools (pool_address) ON DELETE CASCADE,

    -- Fee-split percents (0..=100), resolved from the on-chain cp-amm `Pool`
    -- account by yog-context. Written as a unit — all three are NULL together
    -- until that resolution happens. See migration 018.
    protocol_fee_percent SMALLINT,
    partner_fee_percent  SMALLINT,
    referral_fee_percent SMALLINT,

    -- Fee *shape*, decoded from the genesis PoolFeeParameters blob. See
    -- migration 027 for the closed value set of base_fee_kind.
    base_fee_kind        TEXT,
    has_dynamic_fee      BOOLEAN
);

-- Backfill every row that carries at least one value. Filtered on the values
-- rather than on `protocol`: only cp-amm code paths ever wrote these columns,
-- so this captures all existing data without assuming the protocol label is
-- consistent — the data-preserving choice, since the DROP follows immediately.
INSERT INTO meteora_damm_v2_pool_properties
    (pool_address, protocol_fee_percent, partner_fee_percent,
     referral_fee_percent, base_fee_kind, has_dynamic_fee)
SELECT pool_address, protocol_fee_percent, partner_fee_percent,
       referral_fee_percent, base_fee_kind, has_dynamic_fee
FROM pools
WHERE protocol_fee_percent IS NOT NULL
   OR partner_fee_percent  IS NOT NULL
   OR referral_fee_percent IS NOT NULL
   OR base_fee_kind        IS NOT NULL
   OR has_dynamic_fee      IS NOT NULL;

-- `pools` returns to the cross-protocol registry it was in 001, plus fee_bps.
-- The per-column `GRANT UPDATE (...) TO yog_context` of migrations 018 and 027
-- disappear with the columns — nothing to revoke.
ALTER TABLE pools
    DROP COLUMN protocol_fee_percent,
    DROP COLUMN partner_fee_percent,
    DROP COLUMN referral_fee_percent,
    DROP COLUMN base_fee_kind,
    DROP COLUMN has_dynamic_fee;

-- yog-context owns this table: it resolves the cp-amm Pool account and writes
-- both the neutral pool columns and this satellite from the same read.
-- SELECT is inherited from the default privileges in setup_roles.sql.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_pool_properties TO yog_context;
GRANT SELECT                 ON meteora_damm_v2_pool_properties TO yog_api;
