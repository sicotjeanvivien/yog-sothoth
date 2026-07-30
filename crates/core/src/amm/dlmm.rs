//! DLMM (Meteora Liquidity Book) fee arithmetic.
//!
//! The counterpart of [`super::damm_v2`] for the bin-based product. Only the
//! **base** fee lives here for now — the volatility-driven variable fee needs
//! per-swap state (`volatility_accumulator`) that Yog does not yet track.
//!
//! Source: <https://docs.meteora.ag/core-products/dlmm/formulas>.

use rust_decimal::Decimal;

/// The chain's own ceiling on the total fee rate: `100_000_000` in 1e9
/// precision, i.e. 10 %. Expressed here in the unit this module returns.
///
/// It caps `base_fee_rate + variable_fee_rate`, so it is an upper bound on the
/// base fee alone too — which is what makes the saturation in [`base_fee_bps`]
/// a statement about the protocol rather than a defensive guess.
const MAX_FEE_BPS: Decimal = Decimal::ONE_THOUSAND;

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
/// academic. A pool's variable fee is capped by its own parameters:
///
/// ```text
/// max_variable_fee_bps = ⌈variable_fee_control × (max_volatility_accumulator × bin_step)²
///                         / 1e11⌉ / 1e5
/// ```
///
/// On the accounts captured in `core/tests/fixtures/dlmm/accounts/`:
///
/// ```text
/// pool      base   max variable   worst case
/// DZ2vZJ…    1 bps      0 bps        1 bps   (×1 — no dynamic fee)
/// HTvjzs…    1 bps      2 bps        3 bps   (×3 — the anchor pool)
/// 8KvuP8…  100 bps     70 bps      170 bps   (×1.7)
/// 7t1sXt…  200 bps    675 bps      875 bps   (×4.4)
/// JCYMX9…   25 bps    156 bps      181 bps   (×7.2)
/// ```
///
/// So `fee_bps` ranks and filters pools by a quantity every protocol has, but a
/// DLMM pool at a given tier can charge several times it under volatility, and
/// by a wider factor than a cp-amm pool at the same tier. Callers that need the
/// worst case have what they need: `variable_fee_control`,
/// `max_volatility_accumulator` and `bin_step` are stored raw in the satellite
/// and served raw on the wire, precisely so the bound is recomputable rather
/// than merely asserted here.
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

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "dlmm_tests.rs"]
mod tests;
