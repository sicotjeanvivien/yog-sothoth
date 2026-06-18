use rust_decimal::Decimal;

use crate::CoreResult;
use crate::amm::common::price_impact;
use crate::error::CoreError;

/// cp-amm fee denominator: a fee numerator `n` represents the fraction
/// `n / FEE_DENOMINATOR`. 1e9 → a numerator of 2_500_000 is 0.25 %.
const FEE_DENOMINATOR: u64 = 1_000_000_000;

/// Number of leading bytes of a borsh `PoolFeeParameters` blob occupied by the
/// opaque `BaseFeeParameters` (`[u8; 27]`), which holds the base-fee config.
const BASE_FEE_LEN: usize = 27;

/// Byte offset, within the base-fee blob, of the `BaseFeeMode` discriminant.
const BASE_FEE_MODE_OFFSET: usize = 26;

/// Decode the pool's base trading fee, in basis points, from the raw borsh
/// `PoolFeeParameters` blob captured at pool genesis under "voie C"
/// (`MeteoraDammV2InitializePoolEvent::pool_fees_raw`).
///
/// The base fee lives in the leading opaque `[u8; 27]` `BaseFeeParameters`
/// blob, whose interpretation depends on the `BaseFeeMode` discriminant at
/// byte [`BASE_FEE_MODE_OFFSET`]:
///   - `0` — fee scheduler, linear decay
///   - `1` — fee scheduler, exponential decay
///   - `2` — rate limiter (anti-sniper)
///
/// In all three the base / cliff fee numerator is the leading little-endian
/// `u64`, which we surface as the headline fee tier. For a scheduler pool this
/// is the fee **at genesis** (the starting, pre-decay rate), not the decayed
/// current value — computing the live decayed rate is deliberately out of
/// scope (it varies per read and ignores the dynamic-fee component).
///
/// Fails loud on a too-short blob or an unknown mode: we never guess an
/// unrecognised layout (the caller skips-and-logs, leaving the fee unknown
/// rather than persisting a wrong value).
pub fn decode_base_fee_bps(pool_fees_raw: &[u8]) -> CoreResult<Decimal> {
    if pool_fees_raw.len() < BASE_FEE_LEN {
        return Err(CoreError::FeeDecode {
            reason: format!(
                "blob too short: {} bytes, need at least {BASE_FEE_LEN}",
                pool_fees_raw.len()
            ),
        });
    }

    let mode = pool_fees_raw[BASE_FEE_MODE_OFFSET];
    if mode > 2 {
        return Err(CoreError::FeeDecode {
            reason: format!("unknown BaseFeeMode discriminant: {mode}"),
        });
    }

    let cliff_fee_numerator = u64::from_le_bytes(
        pool_fees_raw[0..8]
            .try_into()
            .expect("slice is 8 bytes after the length check"),
    );

    Ok(fee_numerator_to_bps(cliff_fee_numerator))
}

/// Decode the new base trading fee (basis points) from an `EvtUpdatePoolFees`
/// `params_raw` blob (borsh `UpdatePoolFeesParameters`), captured raw under
/// "voie C" (`MeteoraDammV2UpdatePoolFeesEvent::params_raw`). Returns `None`
/// when the update did not change the base fee.
///
/// `UpdatePoolFeesParameters` leads with `cliff_fee_numerator: Option<u64>`,
/// so reading only that leading field is **robust to trailing-field schema
/// drift**: the struct has since gained `dynamic_fee` / `compounding_fee_bps`
/// (and a captured fixture predates the latter — its blob is one byte short of
/// the current three-field layout), none of which the headline fee tier needs.
///   - tag `0` → `None` (base fee unchanged by this update)
///   - tag `1` → `Some(bps)` decoded from the following little-endian `u64`
///   - any other tag → fail-loud (a malformed borsh `Option` discriminant)
pub fn decode_updated_base_fee_bps(params_raw: &[u8]) -> CoreResult<Option<Decimal>> {
    let Some((&tag, rest)) = params_raw.split_first() else {
        return Err(CoreError::FeeDecode {
            reason: "empty UpdatePoolFeesParameters blob".to_string(),
        });
    };
    match tag {
        0 => Ok(None),
        1 => {
            if rest.len() < 8 {
                return Err(CoreError::FeeDecode {
                    reason: format!(
                        "cliff_fee_numerator truncated: {} bytes after tag, need 8",
                        rest.len()
                    ),
                });
            }
            let numerator = u64::from_le_bytes(
                rest[0..8]
                    .try_into()
                    .expect("8 bytes after the length check"),
            );
            Ok(Some(fee_numerator_to_bps(numerator)))
        }
        other => Err(CoreError::FeeDecode {
            reason: format!("invalid cliff_fee_numerator Option tag: {other}"),
        }),
    }
}

/// Convert a cp-amm fee numerator to basis points. The fee fraction is
/// `numerator / FEE_DENOMINATOR`; in bps that is `numerator / 100_000`. Exact
/// in `Decimal` (e.g. 2_500_000 → 25, 500_000_000 → 5000, 250_000 → 2.5).
///
/// Public because the cliff fee numerator is also read directly (as the leading
/// `u64`) from the on-chain `Pool` account by yog-context, bypassing the borsh
/// event blobs entirely.
pub fn fee_numerator_to_bps(numerator: u64) -> Decimal {
    Decimal::from(numerator) / Decimal::from(FEE_DENOMINATOR / 10_000)
}

/// Apply the DAMM v2 fee to an input amount.
///
/// Fee is expressed in basis points (1 bp = 0.01%).
/// Returns the amount net of fees.
pub fn fee_adjusted_amount(amount_in: u128, fee_bps: u32) -> CoreResult<u128> {
    let fee = amount_in.checked_mul(fee_bps as u128).ok_or_else(|| {
        crate::error::CoreError::ArithmeticOverflow {
            context: "fee_adjusted_amount: amount_in * fee_bps overflows".to_string(),
        }
    })? / 10_000;

    Ok(amount_in.saturating_sub(fee))
}

/// Compute the net price impact of a DAMM v2 swap, after fees.
///
/// DAMM v2 applies fees before the swap is executed — the effective
/// amount_in used for the x·y=k calculation is amount_in net of fees.
pub fn net_price_impact(
    reserve_a: u128,
    reserve_b: u128,
    amount_in: u128,
    fee_bps: u32,
) -> CoreResult<u32> {
    let amount_in_net = fee_adjusted_amount(amount_in, fee_bps)?;
    price_impact(reserve_a, reserve_b, amount_in_net)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "damm_v2_tests.rs"]
mod tests;
