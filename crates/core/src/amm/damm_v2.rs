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
mod tests {
    use super::*;

    // ── fee_adjusted_amount ─────────────────────────────────────────────────

    #[test]
    fn test_fee_adjusted_amount_25_bps() {
        // 25 bps = 0.25% fee on 1_000_000
        // fee = 1_000_000 * 25 / 10_000 = 2_500
        // net = 997_500
        let result = fee_adjusted_amount(1_000_000, 25).unwrap();
        assert_eq!(result, 997_500);
    }

    #[test]
    fn test_fee_adjusted_amount_zero_fee() {
        // 0 bps → no fee, amount unchanged
        let result = fee_adjusted_amount(1_000_000, 0).unwrap();
        assert_eq!(result, 1_000_000);
    }

    #[test]
    fn test_fee_adjusted_amount_max_fee() {
        // 10_000 bps = 100% fee → net = 0
        let result = fee_adjusted_amount(1_000_000, 10_000).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_fee_adjusted_amount_real_swap() {
        // 0.04 SOL = 40_128_368 native units, fee = 25 bps
        // fee = 40_128_368 * 25 / 10_000 = 100_320
        // net = 40_028_048
        let result = fee_adjusted_amount(40_128_368, 25).unwrap();
        assert_eq!(result, 40_028_048);
    }

    // ── net_price_impact ────────────────────────────────────────────────────

    #[test]
    fn test_net_price_impact_small_swap() {
        // Same values as live swap observed in pipeline
        // 0.04 SOL into pool with 85 SOL reserve
        let reserve_a = 85_301_211_438u128; // post-swap reserve_a
        let reserve_b = 3_167_919_281u128; // post-swap reserve_b
        let amount_in = 40_128_368u128;
        let impact = net_price_impact(reserve_a, reserve_b, amount_in, 25).unwrap();
        // small swap → low impact, expect < 10 bps
        assert!(impact < 10, "expected impact < 10 bps, got {impact}");
    }

    #[test]
    fn test_net_price_impact_higher_than_without_fee() {
        // net impact with fee should be lower than without fee
        // because fee reduces effective amount_in
        let reserve_a = 1_000_000u128;
        let reserve_b = 1_000_000u128;
        let amount_in = 100_000u128;

        let impact_with_fee = net_price_impact(reserve_a, reserve_b, amount_in, 100).unwrap();
        let impact_without_fee =
            crate::amm::common::price_impact(reserve_a, reserve_b, amount_in).unwrap();

        // with fee → less amount_in effective → lower impact
        assert!(
            impact_with_fee <= impact_without_fee,
            "impact_with_fee={impact_with_fee} should be <= impact_without_fee={impact_without_fee}"
        );
    }

    // ── decode_base_fee_bps ─────────────────────────────────────────────────

    /// Real `base_fee` bytes captured from `damm_v2_initialize_pool_2.json`:
    /// a constant-fee pool, cliff_fee_numerator = 2_500_000 → 0.25 % = 25 bps,
    /// mode 0 (linear scheduler, no periods).
    #[test]
    fn decode_base_fee_bps_constant_25bps() {
        let data: [u8; 27] = [
            160, 37, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(
            decode_base_fee_bps(&data).unwrap(),
            Decimal::new(25, 0),
            "2_500_000 / 1e9 = 0.25% = 25 bps"
        );
    }

    /// Real `base_fee` bytes from `damm_v2_initialize_pool.json`: an anti-sniper
    /// fee-scheduler pool starting at 50% — cliff_fee_numerator = 500_000_000 →
    /// 5000 bps. We surface the genesis cliff, not the decayed value.
    #[test]
    fn decode_base_fee_bps_scheduler_cliff_5000bps() {
        let data: [u8; 27] = [
            0, 101, 205, 29, 0, 0, 0, 0, 144, 0, 88, 2, 0, 0, 0, 0, 0, 0, 196, 159, 46, 0, 0, 0, 0,
            0, 0,
        ];
        assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(5000, 0));
    }

    /// A fractional sub-bps fee must not round: 250_000 / 1e9 = 0.000_25 = 2.5 bps.
    #[test]
    fn decode_base_fee_bps_fractional_is_lossless() {
        let mut data = [0u8; 27];
        data[0..8].copy_from_slice(&250_000u64.to_le_bytes());
        assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(25, 1));
    }

    /// Rate-limiter mode (2) is accepted: the base-fee numerator is still the
    /// leading u64.
    #[test]
    fn decode_base_fee_bps_rate_limiter_mode_ok() {
        let mut data = [0u8; 27];
        data[0..8].copy_from_slice(&1_000_000u64.to_le_bytes());
        data[BASE_FEE_MODE_OFFSET] = 2;
        assert_eq!(decode_base_fee_bps(&data).unwrap(), Decimal::new(10, 0));
    }

    /// An unknown mode discriminant is rejected fail-loud — never guessed.
    #[test]
    fn decode_base_fee_bps_unknown_mode_errors() {
        let mut data = [0u8; 27];
        data[BASE_FEE_MODE_OFFSET] = 7;
        assert!(matches!(
            decode_base_fee_bps(&data),
            Err(CoreError::FeeDecode { .. })
        ));
    }

    /// A truncated blob is rejected fail-loud rather than indexing past the end.
    #[test]
    fn decode_base_fee_bps_too_short_errors() {
        assert!(matches!(
            decode_base_fee_bps(&[0u8; 10]),
            Err(CoreError::FeeDecode { .. })
        ));
    }

    // ── decode_updated_base_fee_bps ─────────────────────────────────────────

    /// Real `params_raw` bytes from `damm_v2_update_pool_fees.json`:
    /// cliff_fee_numerator = Some(12_800_000) → 128 bps, followed by a
    /// dynamic_fee (Some) and NO compounding_fee_bps field (the tx predates it)
    /// — which the leading-field decode ignores.
    #[test]
    fn decode_updated_base_fee_bps_real_fixture_128bps() {
        let params: [u8; 42] = [
            1, 0, 80, 195, 0, 0, 0, 0, 0, 1, 1, 0, 203, 16, 199, 186, 184, 141, 6, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 10, 0, 120, 0, 136, 19, 96, 164, 220, 0, 200, 4, 0, 0,
        ];
        assert_eq!(
            decode_updated_base_fee_bps(&params).unwrap(),
            Some(Decimal::new(128, 0)),
            "12_800_000 / 1e9 = 1.28% = 128 bps"
        );
    }

    /// tag 0 → the update left the base fee untouched.
    #[test]
    fn decode_updated_base_fee_bps_none_when_unchanged() {
        // cliff None, then whatever trailing bytes — ignored.
        let params = [0u8, 1, 2, 3, 4];
        assert_eq!(decode_updated_base_fee_bps(&params).unwrap(), None);
    }

    /// tag 1 but fewer than 8 trailing bytes → fail-loud.
    #[test]
    fn decode_updated_base_fee_bps_truncated_value_errors() {
        assert!(matches!(
            decode_updated_base_fee_bps(&[1, 0, 0, 0]),
            Err(CoreError::FeeDecode { .. })
        ));
    }

    /// A non-0/1 Option discriminant is rejected.
    #[test]
    fn decode_updated_base_fee_bps_bad_tag_errors() {
        assert!(matches!(
            decode_updated_base_fee_bps(&[9, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(CoreError::FeeDecode { .. })
        ));
    }

    /// An empty blob is rejected.
    #[test]
    fn decode_updated_base_fee_bps_empty_errors() {
        assert!(matches!(
            decode_updated_base_fee_bps(&[]),
            Err(CoreError::FeeDecode { .. })
        ));
    }

    #[test]
    fn test_net_price_impact_zero_fee_equals_price_impact() {
        // 0 bps fee → net_price_impact == price_impact
        let reserve_a = 1_000_000u128;
        let reserve_b = 1_000_000u128;
        let amount_in = 50_000u128;

        let net = net_price_impact(reserve_a, reserve_b, amount_in, 0).unwrap();
        let raw = crate::amm::common::price_impact(reserve_a, reserve_b, amount_in).unwrap();

        assert_eq!(net, raw);
    }
}
