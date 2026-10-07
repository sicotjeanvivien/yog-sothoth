//! When [`UnpricedMints`] asks again. Mutations these are written against:
//! no reset (`a_price_puts_the_mint_back_on_every_tick`), an answer that moves
//! a mint it did not mention (`a_mint_the_source_said_nothing_about_keeps_its_schedule`),
//! no cap, and a wait counted from the answer rather than one tick early
//! (`the_wait_doubles_then_stops_at_the_cap`).

use super::*;

/// The rule at the default cadence.
const TICK: core::time::Duration = core::time::Duration::from_secs(30);

/// How long after its tick began an answer arrives: past the tick boundary.
const ANSWER_DELAY: Duration = Duration::milliseconds(100);

fn t0() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).expect("valid timestamp")
}

fn mint(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Seconds from the tick that asked `mint`, at `asked`, to the next tick that
/// asks it — ticks every `cadence`, as the worker sees them.
fn next_ask(rule: &UnpricedMints, mint: &Pubkey, asked: DateTime<Utc>, cadence: Duration) -> i64 {
    let mut tick = asked + cadence;
    while !rule.is_due(mint, tick) {
        tick += cadence;
        assert!(tick - asked <= Duration::days(1), "never asked again");
    }
    (tick - asked).num_seconds()
}

/// The spacing between the asks of a mint answered `misses` times in a row
/// without a price.
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
    // Exactly 1, 2, 4, 8, then 15 minutes — not a tick later each time.
    assert_eq!(spacings(30, 7), vec![60, 120, 240, 480, 900, 900, 900]);
}

#[test]
fn the_wait_is_counted_in_ticks() {
    // At five minutes, the cap is reached at the second answer.
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

    // The next answer is about other mints, as for a chunk given up on 429.
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

    // Far past the cap: the exponent must not overflow.
    for _ in 0..100 {
        rule.record([], &[dead], t0() + ANSWER_DELAY);
    }

    assert_eq!(next_ask(&rule, &dead, t0(), step), 900);
}

#[test]
fn the_bound_stated_at_startup_is_the_cap_or_the_cadence() {
    assert_eq!(
        UnpricedMints::new(TICK).asks_again_at_most_every(),
        Duration::minutes(15)
    );
    assert_eq!(
        UnpricedMints::new(core::time::Duration::from_secs(1_200)).asks_again_at_most_every(),
        Duration::minutes(20),
        "above the cap, every tick asks"
    );
}
