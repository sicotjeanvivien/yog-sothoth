//! What every on-chain event list shares around its queries: the cursor's key as they bind it,
//! and the forward-only page cut from their rows.

use chrono::{DateTime, Utc};
use yog_core::{Cursor, Page, domain::EventCursor};

use super::{PageBuilder, QueryMode};

/// The key a page reads after, `None` throughout on a first page.
pub(crate) struct EventKey {
    pub(crate) at: Option<DateTime<Utc>>,
    pub(crate) signature: Option<String>,
    pub(crate) event_index: Option<i32>,
}

impl EventKey {
    pub(crate) fn of(cursor: Option<&EventCursor>) -> Self {
        Self {
            at: cursor.map(|c| c.at),
            signature: cursor.map(|c| c.signature.to_string()),
            event_index: cursor.map(|c| i32::from(c.event_index)),
        }
    }
}

/// `events`, read with one row past `limit`, cut to it, the next cursor built from the last.
/// ⚠️ Forward only: a previous cursor would be read forward from the first item.
pub(crate) fn finish_event_page<T>(
    events: Vec<T>,
    limit: i64,
    had_cursor: bool,
    cursor: impl Fn(&T) -> Cursor,
) -> Page<T> {
    let page = PageBuilder::new(events, limit, QueryMode::Forward, had_cursor).finalize(cursor);
    Page {
        prev_cursor: None,
        ..page
    }
}
