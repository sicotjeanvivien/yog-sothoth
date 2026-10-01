//! Tests for the stall clock: what counts as the server's silence.
//!
//! Only what `run` cannot show without timing it: how waits add up. That a
//! block-meta resets the clock and a ping does not is owned by `listener_tests`
//! (`pings_do_not_keep_a_stalled_stream_alive`,
//! `a_slow_but_live_stream_is_not_restarted`).

use super::*;

const TIMEOUT: Duration = Duration::from_secs(30);

/// ⚠️ **Time spent outside a wait is not silence.** The listener waits ten
/// seconds, then sits fifty in `handle` on a full consumer, then waits again:
/// only the ten count. A wall-clock reading would see sixty and call a healthy
/// stream stalled on its first ordinary wait after back-pressure.
///
/// Mutation this is written against: counting the gap between two waits —
/// `wait_started` adding the time since the previous `wait_ended`.
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
