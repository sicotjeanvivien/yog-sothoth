-- ============================================================================
-- 039 — meteora_dlmm_pool_properties (the DLMM properties satellite)
-- ============================================================================
-- The DLMM counterpart of migration 036: the pool properties that only exist
-- for the Liquidity Book product, kept out of the cross-protocol `pools`
-- registry.
--
-- ## Why a second satellite rather than columns on `pools`
--
-- The rule 036 established: `pools` holds what every protocol has — address,
-- protocol, token pair, first/last sighting, and the normalized `fee_bps`.
-- Nothing below exists for cp-amm. There is no `bin_step` in a constant-product
-- pool and no fee scheduler in a bin-based one, so a shared table would carry
-- NULL columns for an entire protocol — the shape "voie 3" rejects.
--
-- No backfill: no DLMM pool has ever been enriched, so every row here will be
-- written by yog-context's PoolAccountWorker from a fresh account read.
--
-- ## This table is dormant by construction, and that is not a bug
--
-- It will stay **empty** until DLMM event extraction lands.
-- `MeteoraDlmm::extract_events` is still a stub returning an empty outcome, and
-- pool discovery runs off extracted events (`pool_maintenance`), so no row with
-- `protocol = 'meteora_dlmm'` reaches `pools` — and this satellite's queue has
-- nothing to resolve.
--
-- The table is deliberately laid down ahead of that: the decoder, the resolver
-- and the read path are testable today (see
-- `crates/core/tests/fixtures/dlmm/accounts/`, and seeding one `pools` row by
-- hand resolves it end to end), and landing them separately keeps the DLMM event
-- work from carrying a schema change as well.
--
-- If you are reading this because the table is empty: that is why. Look at the
-- extractor, not here.
--
-- ## Configuration, not state
--
-- Every column is a *parameter*, changed only by an `update_fee_parameters`.
-- The pool's state — `active_id` (the active bin) and the volatility
-- accumulator with its decay — moves on every swap and belongs to
-- `pool_current_state`. Storing it here would rewrite this row on every crossed
-- bin, for a table whose whole point is that it rarely changes.
--
-- ## `fee_bps` stays on `pools`, and now genuinely earns it
--
-- 036 kept `fee_bps` in the registry arguing it is "a normalized cross-protocol
-- notion". That was true in principle and unproven in practice: its only writer
-- was a cp-amm decoder. This migration supplies the second one.
--
--   base_fee_rate = base_factor × bin_step × 10 × 10^base_fee_power_factor  (1e9)
--   fee_bps       = base_factor × bin_step × 10^base_fee_power_factor / 10_000
--
-- Semantically it is the same quantity cp-amm's cliff numerator carries: the
-- **floor** a swapper pays, before the volatility-driven part. So DLMM enters
-- the fee-tier filter (`WHERE fee_bps = $n`) and `list_fee_tiers` on equal
-- terms. See `yog_core::amm::dlmm::base_fee_bps`, and the nine real accounts in
-- `crates/core/tests/fixtures/dlmm/accounts/` that pin it to published tiers
-- (0, 1, 5, 25, 50, 100, 200 bps).
--
-- ## Integer widths are chosen to be lossless
--
-- The on-chain fields are unsigned; Postgres integers are signed. A `u16` does
-- not fit SMALLINT (32 767 < 65 535) and a `u32` does not fit INTEGER, so each
-- column takes the next width up. The alternative — storing the signed
-- reinterpretation — would decode negative fee parameters for the top half of
-- each range.

CREATE TABLE meteora_dlmm_pool_properties (
    pool_address               TEXT     PRIMARY KEY
                                        REFERENCES pools (pool_address) ON DELETE CASCADE,

    -- Price increment between adjacent bins, in bps: bin i sits at
    -- (1 + bin_step / 10_000)^i. The defining property of a DLMM pool — the
    -- analogue of a fee tier — and one of the two inputs to the base fee.
    bin_step                   INTEGER,   -- u16

    -- The other two inputs to `pools.fee_bps`, kept raw so the derivation stays
    -- auditable and recomputable rather than only surviving as its result.
    base_factor                INTEGER,   -- u16
    base_fee_power_factor      SMALLINT,  -- u8

    -- Dynamic-fee parameters. There is no boolean here on purpose: DLMM has no
    -- `has_dynamic_fee` flag, it expresses "no dynamic fee" as
    -- variable_fee_control = 0, so the magnitude carries both facts.
    --   variable_fee_rate = ceil(variable_fee_control
    --                            × (volatility_accumulator × bin_step)² / 1e11)
    variable_fee_control       BIGINT,    -- u32
    max_volatility_accumulator BIGINT,    -- u32

    -- Meteora's cut of the trading fee, in **basis points** — not the whole
    -- percent cp-amm's `protocol_fee_percent` uses. The two are not comparable
    -- without scaling, which is one more reason they live in separate tables.
    protocol_share             INTEGER    -- u16
);

-- Every column is NULL together, for a pool discovered but not yet enriched:
-- they all come from one read of one `LbPair` account. Unlike cp-amm's
-- `base_fee_kind`, none has a partial-failure mode — they are fixed-offset
-- integers with no open enum to recognise — so `bin_step IS NULL` is an exact
-- test for "never resolved", and that is what the resolver's queue predicate
-- keys off.

-- yog-context owns this table: it resolves the LbPair account and writes both
-- the neutral pool columns (through the `pools` repository) and this satellite.
-- SELECT is inherited from the default privileges in setup_roles.sql; the
-- explicit grants below mirror migration 036 rather than relying on it.
GRANT SELECT, INSERT, UPDATE ON meteora_dlmm_pool_properties TO yog_context;
GRANT SELECT                 ON meteora_dlmm_pool_properties TO yog_api;
