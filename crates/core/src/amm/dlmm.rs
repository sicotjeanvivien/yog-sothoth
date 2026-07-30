//! DLMM (Meteora Liquidity Book) fee arithmetic.
//!
//! The counterpart of [`super::damm_v2`] for the bin-based product.
//!
//! Two quantities, both derived from stored pool parameters and neither
//! requiring per-swap state:
//!
//! - [`base_fee_bps`] — the **floor** a swapper pays, what `pools.fee_bps` holds;
//! - [`max_variable_fee_bps`] — the **ceiling** on the volatility-driven part
//!   this pool can add on top.
//!
//! Together they bound what a pool can charge. The *actual* variable fee at any
//! moment needs the live `volatility_accumulator`, which Yog does not track —
//! but its maximum is a property of the pool, and that is enough to say how far
//! two pools at the same fee tier can diverge.
//!
//! Source: <https://docs.meteora.ag/core-products/dlmm/formulas>.

use rust_decimal::Decimal;

/// The chain's own ceiling on the total fee rate: `100_000_000` in 1e9
/// precision, i.e. 10 %. Expressed here in the unit this module returns.
///
/// It caps `base_fee_rate + variable_fee_rate` **at swap time**, so it bounds
/// each part individually too — which is why both functions here saturate at it
/// rather than returning an error.
const MAX_FEE_BPS: Decimal = Decimal::ONE_THOUSAND;

/// Divisor of the variable-fee formula, applied to a value already in 1e9
/// precision.
const VARIABLE_FEE_SCALE: u128 = 100_000_000_000; // 1e11

/// Above this numerator the result exceeds [`MAX_FEE_BPS`], so the arithmetic
/// can stop early rather than risk `u128` overflow on the way there.
///
/// `1000 bps` is `1e8` in 1e9 precision, and the formula divides by 1e11 —
/// so the numerator that lands exactly on the cap is `1e8 × 1e11`.
const VARIABLE_FEE_SATURATION_NUMERATOR: u128 = 100_000_000 * VARIABLE_FEE_SCALE; // 1e19

/// A DLMM pool's **base** trading fee, in basis points.
///
/// ```text
/// base_fee_rate = base_factor × bin_step × 10 × 10^base_fee_power_factor   (1e9 precision)
/// ```
///
/// Dividing by 1e9 gives the fraction and multiplying by 10_000 gives bps, so
/// the two scalings collapse into a single division by 10_000:
///
/// ```text
/// fee_bps = base_factor × bin_step × 10^base_fee_power_factor / 10_000
/// ```
///
/// Meteora pairs a larger `bin_step` with a larger base fee, each bin being a
/// larger price move — which is why the two multiply rather than being read
/// independently.
///
/// # The floor, not the whole fee — and the gap is wider than cp-amm's
///
/// A swapper pays `min(base_fee_rate + variable_fee_rate, 10 %)`, the variable
/// part rising with volatility. This returns the floor only: the **same
/// definition** as [`super::damm_v2::fee_numerator_to_bps`], which is what lets
/// one `pools.fee_bps` column mean one thing across protocols.
///
/// Same definition, **different upper bound** — and the difference is not
/// academic: on real captured accounts a DLMM pool's ceiling runs from ×1 to
/// ×7.25 this floor. So `fee_bps` ranks and filters pools by a quantity every
/// protocol has, but two pools at the same tier are not interchangeable.
///
/// [`max_variable_fee_bps`] computes that ceiling and carries the per-pool
/// figures; they are not restated here, so recapturing a fixture updates one
/// table rather than leaving copies to drift.
///
/// # Total, deliberately
///
/// `base_fee_power_factor` is a `u8`, so `10^255` is representable on-chain in
/// principle and would overflow any fixed-point type — and even without it the
/// raw product tops out at `65_535 × 65_535 / 10_000` ≈ 429 496 bps, far past
/// the cap. The result saturates at [`MAX_FEE_BPS`] instead of returning an
/// error, because the caller has nowhere to put one:
/// `PoolRegistryProperties::fee_bps` is not an `Option`, and a pool that cannot
/// resolve never leaves `PoolAccountResolver::list_unresolved` — it would sit at
/// the head of the queue and starve every pool behind it.
///
/// **The saturation is a clamp on an input the chain should never produce, not a
/// mirror of chain behaviour.** The program caps `base + variable` *at swap
/// time*; it does not normalise stored parameters. An `LbPair` whose raw
/// parameters exceeded this bound would be an abnormal account, and flattening
/// it to 1000 gives it the appearance of a legitimate 10 % tier.
///
/// That is tolerable only because nothing is lost: `base_factor`, `bin_step` and
/// `base_fee_power_factor` are persisted raw next to the derived `fee_bps`, so a
/// clamped value stays reconstructible and detectable after the fact. Unreachable
/// in practice — lb_clmm validates these parameters at init — but the guarantee
/// is "the inputs survive", not "this cannot happen".
///
/// Exact for every realistic input: the arithmetic is integer-valued in
/// `Decimal` until the final division, and 10_000 divides cleanly into it.
/// A `bin_step` of 1 with a low `base_factor` yields sub-bps values (e.g. 0.5),
/// which is why this returns `Decimal` and not an integer.
pub fn base_fee_bps(base_factor: u16, bin_step: u16, base_fee_power_factor: u8) -> Decimal {
    // A u32 product of two u16s — cannot overflow. Only the power factor can.
    let mut fee = Decimal::from(u32::from(base_factor) * u32::from(bin_step));
    let ceiling = MAX_FEE_BPS * Decimal::from(10_000);

    // Applied one factor of ten at a time, stopping as soon as the running
    // value is past the cap. Repeated multiplication rather than `powu`, which
    // lives behind rust_decimal's `maths` feature — a whole feature for one
    // power of ten this loop covers exactly.
    for _ in 0..base_fee_power_factor {
        if fee >= ceiling {
            return MAX_FEE_BPS;
        }
        fee *= Decimal::TEN;
    }

    (fee / Decimal::from(10_000)).min(MAX_FEE_BPS)
}

/// The **most** a pool's volatility-driven fee can add on top of its base fee,
/// in basis points.
///
/// ```text
/// variable_fee_rate = ⌈variable_fee_control × (volatility_accumulator × bin_step)² / 1e11⌉
/// ```
///
/// The live `volatility_accumulator` is per-swap state Yog does not track, but
/// the pool caps it at `max_volatility_accumulator` — so substituting the cap
/// turns a moving quantity into a **property of the pool**, computable from the
/// three parameters the satellite already stores.
///
/// # Why this exists
///
/// [`base_fee_bps`] and cp-amm's [`super::damm_v2::fee_numerator_to_bps`] share
/// a definition — the floor — which is what lets one `pools.fee_bps` column rank
/// pools across protocols. They do **not** share an upper bound, and for DLMM
/// the gap is wide enough to matter. On the accounts captured in
/// `core/tests/fixtures/dlmm/accounts/`:
///
/// ```text
/// pool      base   max variable    worst case
/// DZ2vZJ…    1 bps        0 bps         1 bps   (×1 — no dynamic fee)
/// HTvjzs…    1 bps        2 bps         3 bps   (×3 — the anchor pool)
/// 8KvuP8…  100 bps  70.3125 bps  170.3125 bps   (×1.703)
/// 7t1sXt…  200 bps      675 bps       875 bps   (×4.375)
/// JCYMX9…   25 bps   156.25 bps    181.25 bps   (×7.25)
/// ```
///
/// Two pools at the same tier are therefore not interchangeable. This function
/// is what makes that statement checkable instead of documentation: every figure
/// above is asserted against real decoded accounts in
/// `tests/pool_account_fixtures.rs`, which is the authority — the ratios here
/// are rounded to three decimals, the bps values are exact.
///
/// The worst case a swapper can pay is
/// `min(base_fee_bps(..) + max_variable_fee_bps(..), 1000)` — the chain caps the
/// **sum** at 10 %, so the two bounds do not simply add without that clamp.
///
/// # Zero means no dynamic fee
///
/// DLMM has no boolean for it, unlike cp-amm's `has_dynamic_fee`: a pool with
/// `variable_fee_control == 0` charges no variable fee at all, and this returns
/// zero for it. The magnitude carries both facts.
///
/// # Total, like [`base_fee_bps`]
///
/// Saturates at [`MAX_FEE_BPS`] rather than failing, for any result past the cap.
///
/// **The arithmetic itself cannot overflow, and by a computable margin.** At the
/// top of the input ranges:
///
/// ```text
/// acc       = (2³²−1) × (2¹⁶−1) =                             281_470_681_677_825
/// acc²                          =         79_225_744_644_179_490_157_096_730_625
/// acc² × (2³²−1)                = 340_271_982_168_772_322_334_504_870_185_799_909_375
/// u128::MAX                     = 340_282_366_920_938_463_463_374_607_431_768_211_455
/// ```
///
/// The worst case is `0.999969` of `u128::MAX` — 0.003 % of headroom, but
/// headroom the types guarantee. The `checked_mul`s below are therefore *not*
/// guarding a live possibility; they cost nothing and turn a future widening of
/// any input type into the cap instead of a silent wrap in release builds.
pub fn max_variable_fee_bps(
    variable_fee_control: u32,
    max_volatility_accumulator: u32,
    bin_step: u16,
) -> Decimal {
    // (max_volatility_accumulator × bin_step)² × variable_fee_control, in u128
    // because the middle term alone exceeds u64. Cannot overflow at these input
    // widths (see the margin above); checked against the day one of them grows.
    let accumulated = u128::from(max_volatility_accumulator) * u128::from(bin_step);
    let Some(squared) = accumulated.checked_mul(accumulated) else {
        return MAX_FEE_BPS;
    };
    let Some(numerator) = squared.checked_mul(u128::from(variable_fee_control)) else {
        return MAX_FEE_BPS;
    };

    // Past the cap the exact value carries no information — and stopping here
    // is also what keeps the `div_ceil` below away from the u128 boundary.
    if numerator > VARIABLE_FEE_SATURATION_NUMERATOR {
        return MAX_FEE_BPS;
    }

    // The formula rounds **up** — a swapper never pays less than the rate.
    let rate_1e9 = numerator.div_ceil(VARIABLE_FEE_SCALE);

    // Guarded above, so this is at most 1e8 and fits comfortably.
    let rate = u64::try_from(rate_1e9).expect("bounded by the saturation check above");

    // 1e9 precision to bps: divide by 1e5. Sub-bps values are real (the anchor
    // pool's floor is 1 bps), so the fraction is kept.
    Decimal::from(rate) / Decimal::from(100_000)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "dlmm_tests.rs"]
mod tests;
