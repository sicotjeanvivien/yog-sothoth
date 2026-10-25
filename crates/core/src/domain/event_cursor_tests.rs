use chrono::{Duration, TimeZone, Utc};
use solana_signature::Signature;

use super::{EventCursor, EventScope};
use crate::domain::Window;

/// Mutation: the start recomputed from `now` on a later page — the window slides under the
/// pages and drops the events between.
#[test]
fn a_later_page_keeps_the_start_of_its_cursor() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let cursor = EventCursor {
        scope: EventScope::All,
        from: now - Duration::hours(30),
        at: now - Duration::hours(2),
        signature: Signature::from([1; 64]),
        event_index: 0,
    };

    assert_eq!(
        EventCursor::window_start(None, Window::Day, now),
        now - Duration::hours(24)
    );
    assert_eq!(
        EventCursor::window_start(Some(&cursor), Window::Day, now),
        now - Duration::hours(30)
    );
}
