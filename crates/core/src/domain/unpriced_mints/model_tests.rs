//! When [`UnpricedMints`] asks again, and the four ways the rule can be got
//! wrong.
//!
//! - **forget the reset** and a mint that regained its price stays on the
//!   waits of its dead period: up to 15 minutes before its next price, and the
//!   longest wait again at its first miss — `a_price_puts_the_mint_back_on_every_tick`;
//! - **let an answer move a mint it did not mention** and a chunk given up on
//!   429 slows down mints that have a price — `a_mint_the_source_said_nothing_about_keeps_its_schedule`;
//! - **drop the cap** and a mint that comes back to life is asked hours later
//!   — `the_wait_doubles_then_stops_at_the_cap`;
//! - **count the wait from the answer** rather than one tick early, and every
//!   ask lands a tick late — 90 s instead of 60 — which the same test catches,
//!   because it walks the ticks the way the worker does.

use super::*;

/// The rule at the default cadence.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

/// How long after its tick began an answer arrives. Anything above zero puts
/// the answer after the tick boundary, which is the whole point.
const ANSWER_DELAY: Duration = Duration::milliseconds(100);

fn t0() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).expect("valid timestamp")
}

fn mint(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Seconds from the tick that asked `mint`, at `asked`, to the first later tick
/// that asks it again — the ticks falling every `cadence`, as the worker's do
/// when a tick fits its cadence. Every claim below is about this number, what
/// the worker observes, and never about the state that produces it.
fn next_ask(rule: &UnpricedMints, mint: &Pubkey, asked: DateTime<Utc>, cadence: Duration) -> i64 {
    let mut tick = asked + cadence;
    while !rule.is_due(mint, tick) {
        tick += cadence;
        assert!(tick - asked <= Duration::days(1), "never asked again");
    }
    (tick - asked).num_seconds()
}

/// The spacing between the asks of a mint the source answers `misses` times
/// in a row without a price, each answer arriving `ANSWER_DELAY` after the tick
/// that asked.
fn spacings(cadence_secs: u64, misses: usize) -> Vec<i64> {
    let cadence = core::time::Duration::from_secs(cadence_secs);
    let step = Duration::from_std(cadence).expect("small cadence");
    let mut rule = UnpricedMints::new(cadence);
    let dead = mint(1);

    let mut asked = t0();
    let mut spacings = Vec::with_capacity(misses);
    for _ in 0..misses {
        rule.record([], &[dead], asked + ANSWER_DELAY);
        let spacing = next_ask(&rule, &dead, asked, step);
        spacings.push(spacing);
        asked += Duration::seconds(spacing);
    }
    spacings
}

#[test]
fn a_mint_never_answered_is_due() {
    let rule = UnpricedMints::new(TICK);

    assert!(rule.is_due(&mint(1), t0()));
}

#[test]
fn the_wait_doubles_then_stops_at_the_cap() {
    // 1, 2, 4, 8 minutes, then the 15-minute cap for as long as it lasts —
    // exactly, and not a tick later each time.
    assert_eq!(spacings(30, 7), vec![60, 120, 240, 480, 900, 900, 900]);
}

#[test]
fn the_wait_is_counted_in_ticks() {
    // At a five-minute cadence the first wait is already two ticks, and the
    // cap is reached at the second answer.
    assert_eq!(spacings(300, 3), vec![600, 900, 900]);
}

#[test]
fn a_cadence_over_the_cap_asks_every_tick() {
    // Every tick is already further apart than the cap allows.
    assert_eq!(spacings(1_200, 3), vec![1_200, 1_200, 1_200]);
}

#[test]
fn a_price_puts_the_mint_back_on_every_tick() {
    let step = Duration::from_std(TICK).expect("small cadence");
    let mut rule = UnpricedMints::new(TICK);
    let revived = mint(1);

    // Six misses: its wait has reached the cap.
    let mut asked = t0();
    for _ in 0..6 {
        rule.record([], &[revived], asked + ANSWER_DELAY);
        asked += Duration::seconds(next_ask(&rule, &revived, asked, step));
    }

    // Asked at the end of its longest wait, it answers with a price.
    rule.record([&revived], &[], asked + ANSWER_DELAY);
    assert_eq!(
        next_ask(&rule, &revived, asked, step),
        30,
        "a priced mint is asked at the next tick"
    );

    // And a miss after that starts the waits over, not at the cap.
    let next = asked + step;
    rule.record([], &[revived], next + ANSWER_DELAY);
    assert_eq!(
        next_ask(&rule, &revived, next, step),
        60,
        "the first miss after a price waits one minute, not fifteen"
    );
}

#[test]
fn a_mint_the_source_said_nothing_about_keeps_its_schedule() {
    let step = Duration::from_std(TICK).expect("small cadence");
    let mut rule = UnpricedMints::new(TICK);
    let silent = mint(1);
    rule.record([], &[silent], t0() + ANSWER_DELAY);

    // The next tick answers about other mints — the case of a chunk given up on
    // 429, whose mints the caller passes in neither list.
    rule.record([&mint(2)], &[mint(3)], t0() + step + ANSWER_DELAY);

    assert_eq!(
        next_ask(&rule, &silent, t0(), step),
        60,
        "its wait neither restarted nor grew"
    );

    // Its next miss is its second, not its first nor its third.
    let asked = t0() + Duration::seconds(60);
    rule.record([], &[silent], asked + ANSWER_DELAY);
    assert_eq!(next_ask(&rule, &silent, asked, step), 120);
}

#[test]
fn a_mint_without_a_price_for_ever_never_waits_past_the_cap() {
    let step = Duration::from_std(TICK).expect("small cadence");
    let mut rule = UnpricedMints::new(TICK);
    let dead = mint(1);

    // Far more misses than any cadence needs to reach the cap: the exponent
    // must neither overflow nor carry the wait past it.
    for _ in 0..100 {
        rule.record([], &[dead], t0() + ANSWER_DELAY);
    }

    assert_eq!(next_ask(&rule, &dead, t0(), step), 900);
}
