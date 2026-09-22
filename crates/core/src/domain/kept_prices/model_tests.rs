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

/// The rule at the default cadence. `TICK` is named because the floor is
/// `PRICE_SERIES_MAX_GAP - TICK`: the tests below assert the *effective* floor,
/// not the constant, which is the whole point of passing the cadence in.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

fn kept_prices() -> KeptPrices {
    KeptPrices::new(TICK)
}

fn floor() -> Duration {
    PRICE_SERIES_MAX_GAP - Duration::seconds(TICK.as_secs() as i64)
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
    let mint = Pubkey::new_unique();
    let mut kept = kept_prices();
    kept.record(&[at(mint, "1.0", t0())]);

    let just_under = t0() + floor() - Duration::seconds(1);
    assert!(
        !kept.worth_keeping(&at(mint, "1.0", just_under)),
        "one second short of the floor is still a repeat"
    );

    assert!(
        kept.worth_keeping(&at(mint, "1.0", t0() + floor())),
        "at the floor exactly, the row is written — the bound is inclusive"
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
    assert!(kept.worth_keeping(&at(mint, "1.1", mid + floor())));
}

#[test]
fn the_floor_is_set_one_tick_early_so_the_row_lands_before_it() {
    // The defect this constructor exists to remove: a forced row lands at the
    // first tick on or after the floor, so a floor taken literally would space
    // rows by `ceil(gap / interval) × interval` — 16 minutes at a 480 s
    // cadence, past the 15-minute staleness bound, while 300 s and 600 s both
    // stay at 10. Subtracting one tick makes the row land at or *before*
    // PRICE_SERIES_MAX_GAP for every cadence, so there is no ragged set of
    // safe values and no ceiling to enforce.
    let mint = Pubkey::new_unique();

    for secs in [1_u64, 30, 100, 299, 480, 599] {
        let tick = core::time::Duration::from_secs(secs);
        let mut kept = KeptPrices::new(tick);
        kept.record(&[at(mint, "1.0", t0())]);

        // Walk the ticks as the worker does, and find the first one that writes.
        let step = Duration::seconds(secs as i64);
        let mut when = t0();
        loop {
            when += step;
            if kept.worth_keeping(&at(mint, "1.0", when)) {
                break;
            }
            assert!(
                when - t0() < PRICE_SERIES_MAX_GAP,
                "cadence {secs}s: no row written by the floor itself"
            );
        }

        assert!(
            when - t0() <= PRICE_SERIES_MAX_GAP,
            "cadence {secs}s: the forced row landed {} s after the last kept one, \
             past the {} s floor",
            (when - t0()).num_seconds(),
            PRICE_SERIES_MAX_GAP.num_seconds()
        );
    }
}

#[test]
fn a_cadence_at_or_past_the_floor_writes_every_tick() {
    // Nothing left to subtract: the rule goes inert rather than promising a
    // freshness the cadence cannot deliver. The safe direction — a row too
    // many, never one too few.
    let mint = Pubkey::new_unique();
    let mut kept = KeptPrices::new(core::time::Duration::from_secs(
        PRICE_SERIES_MAX_GAP.num_seconds() as u64,
    ));
    kept.record(&[at(mint, "1.0", t0())]);

    assert!(kept.worth_keeping(&at(mint, "1.0", t0() + Duration::seconds(1))));
}
