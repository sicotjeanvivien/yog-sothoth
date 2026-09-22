//! What [`KeptPrices`] keeps, and the three ways the rule can be got wrong.
//!
//! All three fail in silence, and they fail in different directions:
//!
//! - **drop the floor** and a mint whose price never moves keeps one row for
//!   ever, falls out of the 15-minute and one-hour windows of migration 005,
//!   and every USD figure derived from it turns NULL —
//!   `a_motionless_price_is_kept_at_the_floor` is the test that stays red
//!   against that;
//! - **compare the raw `Decimal`s** instead of the stored ones and nothing is
//!   ever suppressed, because 80 % of what the source sends carries more
//!   decimals than the column keeps — `a_difference_below_the_column_scale_is_not_a_change`
//!   is the test for that one;
//! - **take the floor literally**, without subtracting a tick, and the forced
//!   row lands at the first tick *past* it: 16 minutes at a 480 s cadence,
//!   while 300 s and 600 s both stay at 10, so the defect hides between two
//!   safe values — `the_floor_is_set_one_tick_early_so_the_row_lands_before_it`
//!   walks the cadences and is the test for that.

use super::*;
use std::str::FromStr;

/// The rule at the default cadence.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

fn kept_prices() -> KeptPrices {
    KeptPrices::new(TICK)
}

/// Walk the ticks as the worker does, and return how long the motionless price
/// went unwritten — `None` if no tick in `PRICE_SERIES_MAX_GAP × 2` wrote one.
///
/// Every claim about the floor is a claim about *this* number, which is why the
/// tests go through it rather than reading a threshold: the threshold is what
/// an age is compared against, the row lands at the first tick past it, and the
/// gap between those two is exactly the defect this walk exists to catch.
fn ticks_until_a_motionless_price_is_rewritten(cadence: u64) -> Option<Duration> {
    let mint = Pubkey::new_unique();
    let mut kept = KeptPrices::new(core::time::Duration::from_secs(cadence));
    kept.record(&[at(mint, "1.0", t0())]);

    let step = Duration::seconds(cadence as i64);
    let mut when = t0();
    while when - t0() <= PRICE_SERIES_MAX_GAP * 2 {
        when += step;
        if kept.worth_keeping(&at(mint, "1.0", when)) {
            return Some(when - t0());
        }
    }

    None
}

fn at(mint: Pubkey, price: &str, fetched_at: DateTime<Utc>) -> TokenPrice {
    TokenPrice {
        mint,
        price_usd: Decimal::from_str(price).expect("test literal must parse as Decimal"),
        price_provider: PriceProvider::Jupiter,
        confidence: None,
        fetched_at,
    }
}

/// A fixed instant — the rule reads the candidate's own `fetched_at`, never a
/// clock, so every test below moves time by hand.
fn t0() -> DateTime<Utc> {
    DateTime::from_timestamp(1_758_000_000, 0).expect("valid timestamp")
}

#[test]
fn a_mint_never_priced_is_kept() {
    let kept = kept_prices();

    assert!(kept.worth_keeping(&at(Pubkey::new_unique(), "1.0", t0())));
}

#[test]
fn a_price_that_moved_is_kept_at_once() {
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    assert!(kept.worth_keeping(&at(mint, "1.01", t0() + Duration::seconds(30))));
}

#[test]
fn an_unchanged_price_is_not_kept() {
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    assert!(!kept.worth_keeping(&at(mint, "1.0", t0() + Duration::seconds(30))));
}

#[test]
fn a_motionless_price_is_kept_at_the_floor() {
    // The whole point of the floor: the price has not moved, and the row is
    // written anyway so the series never ages past
    // `yog_price_max_age_latest()`. Removing the floor branch must turn this
    // red — that mutation is the proof, not this assertion on its own.
    //
    // `PRICE_SERIES_MAX_GAP` **exactly**, not one tick short of it. An earlier
    // version compared with `>=`, which wrote the row a whole tick early and
    // read as correct because the test asserted the threshold instead of the
    // spacing: at a 30 s cadence it landed at 570 s, and at 200 s at 400 s —
    // a third more forced rows than the floor asks for.
    assert_eq!(
        ticks_until_a_motionless_price_is_rewritten(TICK.as_secs()),
        Some(PRICE_SERIES_MAX_GAP)
    );
}

#[test]
fn a_difference_below_the_column_scale_is_not_a_change() {
    // `NUMERIC(38, 18)` keeps 18 decimals, so these two land on the same
    // stored row. Comparing the raw `Decimal`s would call it a move and write
    // a second row that repeats the first — which, on the real series where
    // 80 % of values carry exactly 18 decimals, is every row.
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    let candidate = at(mint, "1.0000000000000000001", t0() + Duration::seconds(30));
    assert_ne!(
        candidate.price_usd,
        Decimal::from_str("1.0").unwrap(),
        "the two literals must differ as Decimals, or this test proves nothing"
    );
    assert!(!kept.worth_keeping(&candidate));
}

#[test]
fn a_difference_at_the_column_scale_is_a_change() {
    // One unit at the column's own scale — the smallest move the table can
    // record, and the counterpart of the test above: the rounding must not
    // swallow a real change.
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    assert!(kept.worth_keeping(&at(
        mint,
        "1.000000000000000001",
        t0() + Duration::seconds(30)
    )));
}

#[test]
fn a_change_of_provenance_is_kept() {
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    let fallback = TokenPrice {
        price_provider: PriceProvider::Fallback,
        ..at(mint, "1.0", t0() + Duration::seconds(30))
    };
    assert!(kept.worth_keeping(&fallback));
}

#[test]
fn mints_are_judged_independently() {
    // The map is keyed by mint; a move on one must not make the other's
    // repeat look like news, nor the reverse.
    let moved = Pubkey::new_unique();
    let still = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(moved, "1.0", t0()), at(still, "2.0", t0())]);

    let later = t0() + Duration::seconds(30);
    assert!(kept.worth_keeping(&at(moved, "1.5", later)));
    assert!(!kept.worth_keeping(&at(still, "2.0", later)));
}

#[test]
fn recording_again_moves_the_floor_forward() {
    // The floor is measured from the last row KEPT, not from the first one:
    // a mint that moves every minute must never hit the floor at all.
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    let mid = t0() + Duration::minutes(6);
    kept.record(&[at(mint, "1.1", mid)]);

    assert!(
        !kept.worth_keeping(&at(mint, "1.1", t0() + Duration::minutes(11))),
        "11 minutes after the first row, but only 5 after the last kept one"
    );
    assert!(kept.worth_keeping(&at(mint, "1.1", mid + PRICE_SERIES_MAX_GAP)));
}

#[test]
fn no_cadence_spaces_two_kept_rows_past_the_floor() {
    // The defect the constructor removes: a forced row lands at the first tick
    // *past* the threshold, so a floor taken literally would space rows by
    // `ceil(gap / interval) × interval` — 16 minutes at a 480 s cadence, past
    // the 15-minute staleness bound, while 300 s and 600 s both stayed at 10.
    // The defect hid between two safe values, so this walks cadences on both
    // sides of it rather than sampling round numbers.
    for secs in [1_u64, 30, 100, 200, 299, 300, 301, 400, 480, 599] {
        let gap = ticks_until_a_motionless_price_is_rewritten(secs)
            .unwrap_or_else(|| panic!("cadence {secs}s: no row written at all"));

        assert!(
            gap <= PRICE_SERIES_MAX_GAP,
            "cadence {secs}s: two kept rows {} s apart, past the {} s floor",
            gap.num_seconds(),
            PRICE_SERIES_MAX_GAP.num_seconds()
        );
    }
}

#[test]
fn the_forced_row_lands_as_late_as_the_cadence_allows() {
    // The other half, and the one no assertion covered: landing *early* is
    // just as wrong, it simply fails by writing rows nobody asked for instead
    // of by going stale. With `>=` instead of `>`, a 200 s cadence forced a row
    // at 400 s — a third more forced rows than the floor requires — and every
    // "is it under the floor?" assertion still passed.
    //
    // The expected value is the last tick at or before the floor, which is the
    // definition of "as late as the cadence allows".
    for secs in [1_u64, 30, 100, 200, 299, 300] {
        let step = Duration::seconds(secs as i64);
        let latest = step * (PRICE_SERIES_MAX_GAP.num_seconds() / secs as i64) as i32;

        assert_eq!(
            ticks_until_a_motionless_price_is_rewritten(secs),
            Some(latest),
            "cadence {secs}s: the forced row is early, so rows are being written \
             that the floor does not ask for"
        );
    }
}

#[test]
fn above_half_the_floor_every_tick_writes_and_the_rule_goes_inert() {
    // Where the rule stops saving anything, stated rather than discovered.
    //
    // Two kept rows are at most `PRICE_SERIES_MAX_GAP` apart, so a cadence over
    // half of it leaves no room for a suppressed tick in between: from 301 s up
    // every tick must write. That is correct — freshness beats volume — but it
    // means an operator who raises the cadence past five minutes silently
    // loses the whole point of this rule, with `unchanged_total` flat at 0 and
    // nothing else saying so. `rewrites_at_most_every` is what says it: above
    // the threshold it collapses to the cadence itself.
    let half = PRICE_SERIES_MAX_GAP.num_seconds() / 2;

    assert_eq!(
        ticks_until_a_motionless_price_is_rewritten(half as u64),
        Some(PRICE_SERIES_MAX_GAP),
        "at exactly half the floor, one tick is still suppressed"
    );

    for secs in [half as u64 + 1, 400, 599, 600, 899] {
        let step = Duration::seconds(secs as i64);
        assert_eq!(
            ticks_until_a_motionless_price_is_rewritten(secs),
            Some(step),
            "cadence {secs}s: the first tick must write — nothing can be suppressed"
        );
        assert_eq!(
            KeptPrices::new(core::time::Duration::from_secs(secs)).rewrites_at_most_every(),
            step,
            "and the worker must announce the cadence, not a floor it cannot honour"
        );
    }
}

#[test]
fn the_announced_spacing_is_the_one_observed() {
    // `rewrites_at_most_every` is logged at startup and is the only number an
    // operator gets. It is derived arithmetically while the walk above steps
    // tick by tick, so nothing but this test keeps the two honest — and the
    // `>=` defect made them disagree by exactly one tick.
    for secs in [1_u64, 30, 100, 200, 299, 300, 301, 480, 599] {
        let announced =
            KeptPrices::new(core::time::Duration::from_secs(secs)).rewrites_at_most_every();

        assert_eq!(
            ticks_until_a_motionless_price_is_rewritten(secs),
            Some(announced),
            "cadence {secs}s: the worker announces {} s and the rule does something else",
            announced.num_seconds()
        );
    }
}
