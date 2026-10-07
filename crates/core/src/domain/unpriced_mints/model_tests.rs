//! When [`UnpricedMints`] asks again, and the three ways the rule can be got
//! wrong.
//!
//! - **forget the reset** and a mint that regained its price stays on the
//!   waits of its dead period: up to 15 minutes before its next price, and the
//!   longest wait again at its first miss — `a_price_puts_the_mint_back_on_every_tick`;
//! - **let an answer move a mint it did not mention** and a chunk given up on
//!   429 slows down mints that have a price — `a_mint_the_source_said_nothing_about_keeps_its_schedule`;
//! - **drop the cap** and a mint that comes back to life is asked hours later
//!   — `the_wait_doubles_then_stops_at_the_cap`.

use super::*;

/// The rule at the default cadence.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

fn t0() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).expect("valid timestamp")
}

fn mint(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// How long `mint` stays not due from `from`, in seconds — walked one second
/// at a time through `is_due`, so every claim below is about what the worker
/// observes and never about the state that produces it.
fn wait_from(rule: &UnpricedMints, mint: &Pubkey, from: DateTime<Utc>) -> i64 {
    let mut t = from;
    while !rule.is_due(mint, t) {
        t += Duration::seconds(1);
        assert!(t - from <= Duration::days(1), "the mint is never due again");
    }
    (t - from).num_seconds()
}

/// The waits a mint goes through when the source answers it `misses` times in
/// a row without a price, each answer arriving the moment the previous wait
/// ends — as the worker would see it.
fn waits_through(rule: &mut UnpricedMints, mint: Pubkey, misses: usize) -> Vec<i64> {
    let mut now = t0();
    let mut waits = Vec::with_capacity(misses);
    for _ in 0..misses {
        rule.record([], &[mint], now);
        let wait = wait_from(rule, &mint, now);
        waits.push(wait);
        now += Duration::seconds(wait);
    }
    waits
}

#[test]
fn a_mint_never_answered_is_due() {
    let rule = UnpricedMints::new(TICK);

    assert!(rule.is_due(&mint(1), t0()));
}

#[test]
fn the_wait_doubles_then_stops_at_the_cap() {
    let mut rule = UnpricedMints::new(TICK);

    // 1, 2, 4, 8 minutes, then the 15-minute cap for as long as it lasts.
    assert_eq!(
        waits_through(&mut rule, mint(1), 7),
        vec![60, 120, 240, 480, 900, 900, 900]
    );
}

#[test]
fn the_wait_is_counted_in_ticks() {
    // At a five-minute cadence the first wait is already two ticks, and the
    // cap is reached at the second answer.
    let mut rule = UnpricedMints::new(core::time::Duration::from_secs(300));

    assert_eq!(waits_through(&mut rule, mint(1), 3), vec![600, 900, 900]);
}

#[test]
fn a_price_puts_the_mint_back_on_every_tick() {
    let mut rule = UnpricedMints::new(TICK);
    let revived = mint(1);
    waits_through(&mut rule, revived, 6);

    // Asked at the end of its longest wait, it answers with a price.
    let back = t0() + Duration::hours(1);
    rule.record([&revived], &[], back);

    assert!(rule.is_due(&revived, back), "a priced mint is due at once");
    assert!(rule.is_due(&revived, back + Duration::seconds(30)));

    // And a miss after that starts the waits over, not at the cap.
    rule.record([], &[revived], back + Duration::seconds(30));
    assert_eq!(
        wait_from(&rule, &revived, back + Duration::seconds(30)),
        60,
        "the first miss after a price waits one minute, not fifteen"
    );
}

#[test]
fn a_mint_the_source_said_nothing_about_keeps_its_schedule() {
    let mut rule = UnpricedMints::new(TICK);
    let silent = mint(1);
    rule.record([], &[silent], t0());

    // A later answer about other mints — the case of a chunk given up on 429,
    // whose mints the caller passes in neither list.
    let later = t0() + Duration::seconds(30);
    rule.record([&mint(2)], &[mint(3)], later);

    assert_eq!(
        wait_from(&rule, &silent, t0()),
        60,
        "its wait neither restarted nor grew"
    );

    // Its next miss is its second, not its first nor its third.
    let next = t0() + Duration::seconds(60);
    rule.record([], &[silent], next);
    assert_eq!(wait_from(&rule, &silent, next), 120);
}

#[test]
fn a_mint_without_a_price_for_ever_never_waits_past_the_cap() {
    let mut rule = UnpricedMints::new(TICK);
    let dead = mint(1);

    // Far more misses than any cadence needs to reach the cap: the exponent
    // must neither overflow nor carry the wait past it.
    for _ in 0..100 {
        rule.record([], &[dead], t0());
    }

    assert_eq!(wait_from(&rule, &dead, t0()), 900);
}
