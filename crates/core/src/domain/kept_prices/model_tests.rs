//! What [`KeptPrices`] keeps; each of the three ways to get the rule wrong
//! fails in silence, and one test below stays red against each.

use super::*;
use std::str::FromStr;

/// The rule at the default cadence.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

fn kept_prices() -> KeptPrices {
    KeptPrices::new(TICK)
}

/// Walk the ticks as the worker does: how long a motionless price went unwritten.
/// ⚠️ Assert this spacing, never the threshold: the row lands at the next tick.
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

/// A fixed instant: the rule reads `fetched_at`, never a clock.
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
    // Mutation: removing the floor branch, or `>` → `>=` (a row one tick early).
    assert_eq!(
        ticks_until_a_motionless_price_is_rewritten(TICK.as_secs()),
        Some(PRICE_SERIES_MAX_GAP)
    );
}

/// Against the floor back at nine and a half minutes: a mint seen every three ticks of three
/// minutes would be held back at nine, and rewritten at eighteen.
#[test]
fn a_motionless_price_is_written_past_eight_minutes_at_the_default_cadence() {
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1", t0())]);

    assert!(!kept.worth_keeping(&at(mint, "1", t0() + Duration::minutes(8))));
    assert!(kept.worth_keeping(&at(mint, "1", t0() + Duration::seconds(481))));
}

#[test]
fn a_difference_below_the_column_scale_is_not_a_change() {
    // Mutation: comparing the raw `Decimal`s; these two store as the same row.
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
    // One unit at the column's scale: the rounding must not swallow a real change.
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
    // Mutation: keying the map on anything but the mint.
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
    // Mutation: measuring the floor from the first row instead of the last kept.
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
    // Mutation: a floor taken literally, without the tick subtracted. It hides
    // between safe cadences, so this walks them all rather than round numbers.
    for secs in [1_u64, 30, 100, 200, 254, 255, 256, 400, 480, 509] {
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
    // Mutation: `>` → `>=`, a row one tick early. Expected: the last tick at or
    // before the floor.
    for secs in [1_u64, 30, 100, 200, 254, 255] {
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
    // Above half the floor every tick writes: `rewrites_at_most_every` must
    // collapse to the cadence, the only place an operator sees it.
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
    // Mutation: `rewrites_at_most_every` drifting from the tick-by-tick walk.
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
