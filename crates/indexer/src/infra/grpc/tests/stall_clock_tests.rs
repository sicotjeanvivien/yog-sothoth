//! Tests for the stall clock: how waits add up. What resets it is `listener_tests`'.

use super::*;

const TIMEOUT: Duration = Duration::from_secs(30);

/// ⚠️ Time spent outside a wait, parked on a full consumer, is not silence.
/// Mutation this is written against: `wait_started` adding the time since the previous `wait_ended`.
#[test]
fn time_outside_a_wait_is_not_silence() {
    let t0 = Instant::now();
    let mut clock = StallClock::new(TIMEOUT);

    clock.wait_started(t0);
    clock.wait_ended(t0 + Duration::from_secs(10));
    // Fifty seconds parked on a full consumer: no call at all.
    clock.wait_started(t0 + Duration::from_secs(60));

    assert_eq!(
        clock.remaining(),
        Duration::from_secs(20),
        "only the ten seconds spent waiting on the server count"
    );
}

/// Waits add up across messages that are not block-metas.
#[test]
fn successive_waits_add_up() {
    let t0 = Instant::now();
    let mut clock = StallClock::new(TIMEOUT);

    for second in [0, 10, 20] {
        clock.wait_started(t0 + Duration::from_secs(second));
        clock.wait_ended(t0 + Duration::from_secs(second + 8));
    }

    assert_eq!(clock.remaining(), Duration::from_secs(6));
}

/// A silence longer than the timeout leaves nothing, not an underflow.
#[test]
fn the_remaining_time_never_goes_negative() {
    let t0 = Instant::now();
    let mut clock = StallClock::new(TIMEOUT);

    clock.wait_started(t0);
    clock.wait_ended(t0 + Duration::from_secs(45));

    assert_eq!(clock.remaining(), Duration::ZERO);
}
