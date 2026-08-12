//! Boundary of [`TokenPrice::is_storable`].
//!
//! The values below are not round numbers picked for readability — they sit on
//! either side of the one place the rule can be got wrong. `4e-19` and `5e-19`
//! are both positive `Decimal`s; only the second survives a write to
//! `NUMERIC(38, 18)`. A `price_usd > 0` implementation passes **both**, which is
//! precisely the bug this guards, so `sub_midpoint_is_not_storable` is the test
//! that has to stay red against it.
//!
//! `price_positivity.rs` in `yog-persistence` re-asserts these same values
//! against a live Postgres, so the two halves of the rule cannot drift apart.

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
    // 5e-19 rounds AWAY from zero under Postgres's NUMERIC rules, landing on
    // 1e-18. rust_decimal's default (banker's) would round it to zero instead —
    // this test is what pins the strategy.
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
//
// `NUMERIC(38, 18)` leaves 20 integer digits. A value at or above 10^20 is
// refused by the column TYPE (`22003`), not by the CHECK — so the constraint of
// migration 009 cannot catch it, and only this filter stands between an absurd
// Jupiter response and a batch abort that takes every other mint down.

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
