use axum::{extract::Query, http::Uri};
use chrono::{TimeDelta, Utc};
use solana_signature::Signature;
use yog_core::Cursor;
use yog_core::domain::{EventCursor, EventScope};

use super::super::window_param::{DayWindowParam, EventsWindow, WeekWindowParam, WindowParam};
use super::{EventsParams, ListEventsRequest};
use crate::http::cursor::{EventCollection, encode_cursor};
use crate::http::error::ApiError;

fn query<W: EventsWindow>(query: &str) -> Option<EventsParams<W>> {
    let uri: Uri = format!("/api/swaps?{query}").parse().unwrap();
    Query::<EventsParams<W>>::try_from_uri(&uri)
        .ok()
        .map(|Query(params)| params)
}

/// A swaps cursor whose window started `days` ago.
fn cursor_from(days: i64) -> String {
    encode_cursor(&Cursor::SwapEvent(Box::new(EventCursor {
        scope: EventScope::All,
        from: Utc::now() - TimeDelta::days(days),
        at: Utc::now() - TimeDelta::minutes(5),
        signature: Signature::default(),
        event_index: 0,
    })))
    .unwrap()
}

fn accepts_cursor<W: EventsWindow>(days: i64) -> bool {
    let params = query::<W>(&format!("cursor={}", cursor_from(days))).unwrap();
    match ListEventsRequest::parse(params, EventCollection::Swaps, EventScope::All) {
        Ok(_) => true,
        Err(ApiError::BadRequest(message)) => {
            assert!(message.contains("not one this list serves"), "{message}");
            false
        }
        Err(other) => panic!("{other:?}"),
    }
}

// Mutation: `LONGEST` apart from the variants — a cursor bound wider or narrower than the windows the
// type lets through.
#[test]
fn the_longest_window_is_the_widest_one_served() {
    fn widest_served<W: EventsWindow>() -> Option<i32> {
        ["1h", "24h", "7d", "30d"]
            .into_iter()
            .filter_map(|value| query::<W>(&format!("window={value}")))
            .map(|params| params.window.into().hours())
            .max()
    }
    assert_eq!(
        widest_served::<DayWindowParam>(),
        Some(DayWindowParam::LONGEST.hours())
    );
    assert_eq!(
        widest_served::<WeekWindowParam>(),
        Some(WeekWindowParam::LONGEST.hours())
    );
    assert_eq!(
        widest_served::<WindowParam>(),
        Some(WindowParam::LONGEST.hours())
    );
}

// Mutation: the cursor bounded by a fixed window — a forged cursor reads 30 days of a 24-hour list.
#[test]
fn a_cursor_is_bounded_by_its_list_s_window() {
    assert!(!accepts_cursor::<DayWindowParam>(2));
    assert!(accepts_cursor::<WeekWindowParam>(2));
    assert!(!accepts_cursor::<WeekWindowParam>(8));
    assert!(accepts_cursor::<WindowParam>(8));
}
