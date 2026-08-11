//! Constant-product AMM formulas — **dormant, and wrong for the protocols this
//! project indexes.**
//!
//! Every function here models `x·y=k` over the pool's *total* reserves. DAMM v2
//! and DLMM are concentrated-liquidity AMMs: the same reserves back a bounded
//! price range, so a trade moves the price far more than these formulas say.
//! The error has a direction — **x·y=k understates the impact, so the answer is
//! optimistic, not merely inaccurate**. A detector wired to one of these would
//! publish confident, reassuringly low numbers.
//!
//! Kept rather than deleted, deliberately, with the warning attached at each
//! definition: the derivation is real work and the trap is documented where the
//! next detector author will look. The correct formulation for concentrated
//! liquidity is `ΔA = L(1/√P − 1/√P_max)`, which needs the pool's `L` and price
//! bounds — neither of which any event carries.
//!
//! **Nothing outside this module tree calls any of this**, and that is the
//! intended state: the only caller of [`price_impact`] is
//! [`super::damm_v2::net_price_impact`], which is dormant for the same reason.
//! No binary, repository, detector or DTO reaches either. Before wiring one in,
//! replace the model; do not reuse the name.

use crate::{CoreError, CoreResult};

/// Spot price of token A in terms of token B, as a Q64 fixed-point integer.
///
/// Formula: price_q64 = (reserve_b << 64) / reserve_a
///
/// Convert to f64 for display only:
///   price = price_q64 as f64 / (1u128 << 64) as f64
///
/// ⚠️ **The price of a concentrated-liquidity pool is not the ratio of its
/// reserves** — see the module note. The function that answers this question
/// correctly is [`super::damm_v2::sqrt_price_to_price_a_in_b`], which reads the
/// pool's own `sqrt_price`, and it is the one every caller uses. This one is
/// the base of [`price_impact`] and shares its dormancy.
pub fn spot_price(reserve_a: u128, reserve_b: u128) -> CoreResult<u128> {
    if reserve_a == 0 {
        return Err(CoreError::ArithmeticOverflow {
            context: "spot_price: reserve_a is zero".to_string(),
        });
    }

    let numerator = reserve_b
        .checked_shl(64)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "spot_price: reserve_b << 64 overflows u128".to_string(),
        })?;

    Ok(numerator / reserve_a)
}

/// Price impact of a swap in basis points (1 bp = 0.01%).
///
/// Formula: impact_bps = ((price_after - price_before) / price_before) * 10_000
///
/// Uses Q64 prices to stay in integer arithmetic throughout.
///
/// ⚠️ **Dormant and optimistic** — see the module note. This is the constant
/// product model applied to vault totals; on a concentrated-liquidity pool the
/// true impact is larger, so a signal built on it would under-report exactly
/// the events worth alerting on.
pub fn price_impact(reserve_a: u128, reserve_b: u128, amount_in: u128) -> CoreResult<u32> {
    let price_before = spot_price(reserve_a, reserve_b)?;

    let reserve_a_after =
        reserve_a
            .checked_add(amount_in)
            .ok_or_else(|| CoreError::ArithmeticOverflow {
                context: "price_impact: reserve_a + amount_in overflows".to_string(),
            })?;

    // x·y=k — reserve_b_after = k / reserve_a_after
    let k = reserve_a
        .checked_mul(reserve_b)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "price_impact: reserve_a * reserve_b overflows".to_string(),
        })?;

    let reserve_b_after = k / reserve_a_after;

    let price_after = spot_price(reserve_a_after, reserve_b_after)?;

    // price_after <= price_before for a standard swap
    let delta = price_before.saturating_sub(price_after);

    let impact_bps = delta
        .checked_mul(10_000)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "price_impact: delta * 10_000 overflows".to_string(),
        })?
        / price_before;

    Ok(impact_bps as u32)
}

/// Pool imbalance in basis points (1 bp = 0.01%): how far the pool deviates
/// from a 50/50 reserve ratio. A perfectly balanced pool returns 0 bps.
///
/// Both reserves must be expressed in the same unit (e.g. USD value) for this
/// metric to be meaningful — the caller is responsible for the conversion, and
/// passing raw token amounts of different decimals yields a number that looks
/// like a ratio and is not one.
///
/// ⚠️ **Dormant, and it does not mean what the signal engine means.** A 50/50
/// reserve split is the resting state of a *constant-product* pool; a
/// concentrated-liquidity position is deliberately lopsided as the price moves
/// through its range, so a large value here is the normal condition of a
/// healthy DAMM v2 pool, not a finding.
///
/// Not to be confused with `FlowImbalanceDetector`, which is live and measures
/// something else entirely: the directional imbalance of **USD flow** between
/// the two swap directions over a window. The shared word is the only thing
/// they have in common.
pub fn imbalance(reserve_a: u128, reserve_b: u128) -> CoreResult<u32> {
    let total = reserve_a
        .checked_add(reserve_b)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "imbalance: reserve_a + reserve_b overflows".to_string(),
        })?;

    if total == 0 {
        return Err(CoreError::ArithmeticOverflow {
            context: "imbalance: total reserves are zero".to_string(),
        });
    }

    let diff = reserve_a.abs_diff(reserve_b);

    let imbalance_bps = diff
        .checked_mul(10_000)
        .ok_or_else(|| CoreError::ArithmeticOverflow {
            context: "imbalance: diff * 10_000 overflows".to_string(),
        })?
        / total;

    Ok(imbalance_bps as u32)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
