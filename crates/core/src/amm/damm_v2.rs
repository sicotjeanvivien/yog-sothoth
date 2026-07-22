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

/// Byte offset, within the base-fee blob, of the fee-scheduler period count
/// (`number_of_period: u16`, little-endian). A count of zero means the base
/// fee never moves — a *constant* fee — even under a scheduler mode; a
/// non-zero count is what makes a mode 0/1 pool an actual decaying scheduler.
/// Only meaningful for modes 0/1: mode 2 (rate limiter) reinterprets these
/// bytes.
const NUMBER_OF_PERIOD_OFFSET: usize = 8;

/// Byte offset, within the full `PoolFeeParameters` blob, of the borsh
/// `Option<DynamicFeeParameters>` tag. It follows the 27-byte base fee, a
/// `u16 compounding_fee_bps` and a `u8` padding: 27 + 2 + 1 = 30. A tag of
/// `1` means a volatility-based dynamic fee sits on top of the base fee.
const DYNAMIC_FEE_TAG_OFFSET: usize = 30;

/// How a DAMM v2 pool's **base** trading fee behaves over time.
///
/// Decoded from the `BaseFeeMode` discriminant plus the scheduler period
/// count — the mode byte alone is not enough, since a scheduler mode with
/// zero periods is a constant fee (see [`decode_fee_config`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseFeeKind {
    /// Fixed fee — no scheduling (modes 0/1 with `number_of_period == 0`).
    Constant,
    /// Fee scheduler with linear decay (mode 0, `number_of_period > 0`).
    SchedulerLinear,
    /// Fee scheduler with exponential decay (mode 1, `number_of_period > 0`).
    SchedulerExponential,
    /// Rate limiter / anti-sniper (mode 2). Its internal parameters are
    /// deliberately not decoded — that layout reuses bytes 8..26 and has no
    /// captured fixture to validate against.
    RateLimiter,
}

impl BaseFeeKind {
    /// Stable, lowercase discriminant for persistence / the wire. Kept in
    /// sync with the DB `base_fee_kind` column values.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::SchedulerLinear => "scheduler_linear",
            Self::SchedulerExponential => "scheduler_exponential",
            Self::RateLimiter => "rate_limiter",
        }
    }
}

/// The decodable shape of a pool's fee configuration.
///
/// Two orthogonal dimensions — a pool can run a base-fee scheduler *and* a
/// volatility dynamic fee at once. Only what the stored genesis blob lets us
/// decode without guessing an unvalidated layout is surfaced here; the live
/// decayed rate, the dynamic-fee magnitude, and the rate-limiter internals
/// are intentionally out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeConfig {
    /// How the base fee behaves over time.
    pub base_kind: BaseFeeKind,
    /// Whether a volatility-based dynamic fee is enabled on top of the base
    /// fee (the `Option<DynamicFeeParameters>` is present).
    pub has_dynamic_fee: bool,
}

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

/// Decode the pool's fee *shape* — base-fee kind and whether a volatility
/// dynamic fee is enabled — from the same genesis `PoolFeeParameters` blob
/// that [`decode_base_fee_bps`] reads (`pool_fees_raw`).
///
/// Companion to `decode_base_fee_bps`: that returns the headline fee *tier*,
/// this returns how the fee *behaves*. The two are read from the same bytes
/// and typically persisted together.
///
/// The base-fee kind comes from the `BaseFeeMode` discriminant (byte
/// [`BASE_FEE_MODE_OFFSET`]) **combined with** the scheduler period count
/// (bytes at [`NUMBER_OF_PERIOD_OFFSET`]): the mode alone cannot tell a
/// constant fee from a scheduler, because a scheduler mode with zero periods
/// is constant. `has_dynamic_fee` is the borsh `Option` tag at
/// [`DYNAMIC_FEE_TAG_OFFSET`].
///
/// Fails loud (never guesses) on a blob too short to hold the dynamic-fee
/// tag, an unknown `BaseFeeMode`, or a malformed `Option` tag — the caller
/// skips-and-logs, leaving the fee shape unknown rather than wrong.
pub fn decode_fee_config(pool_fees_raw: &[u8]) -> CoreResult<FeeConfig> {
    // Need the base fee blob and the dynamic-fee Option tag that follows it.
    if pool_fees_raw.len() <= DYNAMIC_FEE_TAG_OFFSET {
        return Err(CoreError::FeeDecode {
            reason: format!(
                "blob too short: {} bytes, need at least {}",
                pool_fees_raw.len(),
                DYNAMIC_FEE_TAG_OFFSET + 1
            ),
        });
    }

    let mode = pool_fees_raw[BASE_FEE_MODE_OFFSET];
    let number_of_period = u16::from_le_bytes([
        pool_fees_raw[NUMBER_OF_PERIOD_OFFSET],
        pool_fees_raw[NUMBER_OF_PERIOD_OFFSET + 1],
    ]);

    let base_kind = match mode {
        // Scheduler modes with no periods never move → a constant fee.
        0 | 1 if number_of_period == 0 => BaseFeeKind::Constant,
        0 => BaseFeeKind::SchedulerLinear,
        1 => BaseFeeKind::SchedulerExponential,
        // Rate limiter: bytes 8..26 mean something else here, so
        // `number_of_period` above is not consulted for this arm.
        2 => BaseFeeKind::RateLimiter,
        other => {
            return Err(CoreError::FeeDecode {
                reason: format!("unknown BaseFeeMode discriminant: {other}"),
            });
        }
    };

    let has_dynamic_fee = match pool_fees_raw[DYNAMIC_FEE_TAG_OFFSET] {
        0 => false,
        1 => true,
        other => {
            return Err(CoreError::FeeDecode {
                reason: format!("invalid dynamic_fee Option tag: {other}"),
            });
        }
    };

    Ok(FeeConfig {
        base_kind,
        has_dynamic_fee,
    })
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

/// Q64.64 fixed-point scale factor: the on-chain `sqrt_price` encodes
/// `sqrt(price) * 2^64`. As `f64` (exact — 2^64 is a power of two).
const Q64_SCALE: f64 = 18_446_744_073_709_551_616.0; // 2^64

/// Derive a pool's **spot price** — units of token B per 1 unit of token A, in
/// human (decimal-adjusted) terms — from the on-chain Q64.64 `sqrt_price`.
///
/// DAMM v2 is concentrated liquidity (Uniswap-v3 style), so the spot price is
/// carried by `sqrt_price`, **not** by the reserve ratio: reserves reflect
/// *where* liquidity is parked across price ranges, not the active trading
/// price. `sqrt_price` encodes `sqrt(raw_price) * 2^64`, where `raw_price` is
/// token B per token A in their raw on-chain integer units. Squaring undoes the
/// square root, dividing out the `2^64` factor undoes the fixed point, and
/// `10^(decimals_a - decimals_b)` rescales raw units to human units:
///
/// ```text
/// price_a_in_b = (sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)
/// ```
///
/// Computed in `f64`. This is a **display / comparison ratio** (a handful of
/// significant figures), not a token quantity, so the project's lossless-integer
/// rule does not apply: `f64`'s ~15 significant digits far exceed any price's
/// display need, and squaring a `u128` exactly would overflow `Decimal` anyway.
/// Validated against real mainnet pool states (see tests). Returns `None` when
/// the result is not a finite, strictly positive number (a zero / garbage
/// `sqrt_price`, or a magnitude `Decimal` cannot hold).
pub fn sqrt_price_to_price_a_in_b(
    sqrt_price: u128,
    decimals_a: u8,
    decimals_b: u8,
) -> Option<Decimal> {
    let ratio = sqrt_price as f64 / Q64_SCALE;
    let exponent = i32::from(decimals_a) - i32::from(decimals_b);
    let price = ratio * ratio * 10f64.powi(exponent);

    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    Decimal::from_f64_retain(price).map(|d| d.normalize())
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
