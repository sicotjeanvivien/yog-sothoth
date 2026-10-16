//! Boundary of [`TokenPrice::is_storable`], on either side of each column end.
//! Mutation: `price_usd > 0`, which `sub_midpoint_is_not_storable` refuses.

use super::*;
use std::str::FromStr;

fn price(s: &str) -> TokenPrice {
    TokenPrice {
        mint: Pubkey::new_unique(),
        price_usd: Decimal::from_str(s).expect("test literal must parse as Decimal"),
        price_provider: PriceProvider::Jupiter,
        confidence: None,
        fetched_at: Utc::now(),
    }
}

#[test]
fn ordinary_price_is_storable() {
    assert!(price("1.25").is_storable());
}

#[test]
fn smallest_representable_price_is_storable() {
    // 1e-18 — exactly one unit at the column's scale, no rounding involved.
    assert!(price("0.000000000000000001").is_storable());
}

#[test]
fn midpoint_rounds_up_and_is_storable() {
    // Mutation: banker's rounding, which stores 5e-19 as zero; Postgres rounds away.
    assert!(price("0.0000000000000000005").is_storable());
}

#[test]
fn sub_midpoint_is_not_storable() {
    // 4e-19: positive, and stored as exactly 0. The case a `> 0` check misses.
    assert!(!price("0.0000000000000000004").is_storable());
}

#[test]
fn far_below_scale_is_not_storable() {
    assert!(!price("0.0000000000000000000000001").is_storable());
}

#[test]
fn zero_is_not_storable() {
    assert!(!price("0").is_storable());
}

#[test]
fn negative_is_not_storable() {
    // No source should ever produce one; the rule refuses it rather than
    // relying on that.
    assert!(!price("-1.25").is_storable());
}

// ── The other end of the column ──────────────────────────────────────────────
// ⚠️ At 10^20 the column type refuses the value, not the CHECK: only this filter guards it.

#[test]
fn the_largest_storable_price_is_accepted() {
    // 20 integer digits, just under the ceiling.
    assert!(price("99999999999999999999").is_storable());
}

#[test]
fn the_ceiling_itself_is_not_storable() {
    // Exactly 10^20 — Postgres requires "an absolute value LESS than 10^20".
    assert!(!price("100000000000000000000").is_storable());
}

#[test]
fn an_absurd_price_is_not_storable() {
    assert!(!price("1000000000000000000000000").is_storable());
}
