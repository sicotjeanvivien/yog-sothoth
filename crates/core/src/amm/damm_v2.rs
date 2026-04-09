use crate::amm::common::price_impact;
use crate::CoreResult;

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
