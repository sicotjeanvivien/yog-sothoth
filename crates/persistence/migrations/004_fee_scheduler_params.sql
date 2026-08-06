-- ============================================================================
-- 004_fee_scheduler_params.sql — store the decay curve, not just its starting
-- point
-- ============================================================================
-- Addresses `.project` ticket 07. `pools.fee_bps` is derived from
-- `cliff_fee_numerator`, the fee a scheduler pool charges at **period 0**. For a
-- `constant` pool that is the whole truth; for a scheduler it is the maximum of
-- a decreasing curve — the opposite of the floor its written contract implies.
--
-- Measured against Meteora's public API on 3 and 4 August 2026, two concordant
-- readings: ×5 on a `scheduler_linear` pool, **×49** on a `scheduler_exponential`
-- one (5000 bps published for 102 bps actually charged, on ~950 k$ of TVL).
-- 333 pools of 987 (34 %) carry a scheduler.
--
-- ## Why the value could not simply be refreshed
--
-- The decay is a function of time that cp-amm evaluates at each swap. It emits
-- **no event**, so no amount of stream watching sees it, and `pools.needs_refresh`
-- is only raised by an operator `UpdatePoolFees`. The curve has to be decoded
-- once and then evaluated on read.
--
-- ## What these six columns are
--
-- `PodAlignedFeeTimeScheduler`, plus the time origin `Pool` keeps separately:
--
--   * `cliff_fee_numerator`  the curve's starting numerator — the same quantity
--                            `pools.fee_bps` is derived from, stored here so the
--                            evaluation needs no join back;
--   * `number_of_period`     how many periods the decay runs for;
--   * `period_frequency`     one period's length, in the unit below;
--   * `reduction_factor`     per-period decay (subtracted, or applied as
--                            `1 - factor/10_000`);
--   * `activation_point`     where the curve starts;
--   * `activation_type`      **0 = slot, 1 = timestamp** — the unit of the two
--                            preceding columns. All eleven captured mainnet
--                            accounts use 1; the slot branch is real but
--                            unwitnessed.
--
-- ## They are NULL for most pools, and that is a fact rather than a gap
--
-- Only the two time-scheduler modes have a curve. `constant` does not decay,
-- the market-cap schedulers decay on capitalisation (which this project does not
-- have), and `rate_limiter`'s fee *rises* with swap size.
--
-- ⚠️ More sharply: `BaseFeeInfo` is a 32-byte region the modes **reinterpret**,
-- so the bytes these columns come from mean something else entirely under modes
-- 2, 3 and 4. Decoded blindly they yield plausible-looking nonsense — measured
-- on the captured fixtures, mode 4 gives a `period_frequency` of
-- 13 722 280 043 814 587 382 and mode 2 one of 42 520 176 273 600. The decoder
-- gates on the mode; a NULL here is that gate holding, not data missing.
--
-- ## Types
--
-- `number_of_period` is `INTEGER`, not `SMALLINT`: it is a `u16` on chain and
-- `SMALLINT` is signed 16-bit, so anything above 32 767 would not round-trip.
-- The three `BIGINT`s hold `u64`s through the crate's checked `u64 → i64`
-- conversion, which fails loud rather than wrapping.
--
-- ## No GRANT statement
--
-- `meteora_damm_v2_pool_properties` is granted at **table** level
-- (`GRANT SELECT, INSERT, UPDATE … TO yog_context`, baseline §9), so added
-- columns are covered without a new grant — unlike `pools`, whose `yog_context`
-- grants are column-scoped and would need one. `tests/privileges.rs` compares
-- explicit grants and is unaffected.
-- ============================================================================

ALTER TABLE meteora_damm_v2_pool_properties
    ADD COLUMN cliff_fee_numerator BIGINT,
    ADD COLUMN number_of_period    INTEGER,
    ADD COLUMN period_frequency    BIGINT,
    ADD COLUMN reduction_factor    BIGINT,
    ADD COLUMN activation_point    BIGINT,
    ADD COLUMN activation_type     SMALLINT;

COMMENT ON COLUMN meteora_damm_v2_pool_properties.activation_type IS
    'Unit of activation_point and period_frequency: 0 = slot, 1 = timestamp.';
COMMENT ON COLUMN meteora_damm_v2_pool_properties.cliff_fee_numerator IS
    'Fee numerator at period 0 — the curve maximum, NOT what a trader pays once the scheduler has decayed.';
