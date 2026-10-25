//! What every on-chain event list shares around its queries: the cursor's key as they bind it,
//! the forward-only page cut from their rows, and the merge of a list read from several tables.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use yog_core::{Cursor, Page, RepositoryError, RepositoryResult, domain::EventCursor};

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

/// The key of an event in a list merged from several tables, and the table and row it is read
/// from.
pub(crate) struct MergedEventKey {
    pub(crate) kind: String,
    pub(crate) id: i64,
    pub(crate) timestamp: DateTime<Utc>,
}

/// The first and last instants of `keys`, which bound the read of their rows.
pub(crate) fn instants_of(keys: &[MergedEventKey]) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let first = keys.iter().map(|k| k.timestamp).min()?;
    let last = keys.iter().map(|k| k.timestamp).max()?;
    Some((first, last))
}

/// The ids of `keys` read from the table `kind`.
pub(crate) fn ids_of(keys: &[MergedEventKey], kind: &str) -> Vec<i64> {
    keys.iter()
        .filter(|k| k.kind == kind)
        .map(|k| k.id)
        .collect()
}

/// `rows`, keyed by table and id, put back in the order of `keys`, the database's.
/// ⚠️ A key without its row is an integrity error, never a gap in the page.
pub(crate) fn in_key_order<'k, T>(
    keys: &'k [MergedEventKey],
    mut rows: HashMap<(&'k str, i64), T>,
) -> RepositoryResult<Vec<T>> {
    keys.iter()
        .map(|k| {
            rows.remove(&(k.kind.as_str(), k.id)).ok_or_else(|| {
                RepositoryError::Integrity(format!("event {} {} without its row", k.kind, k.id))
            })
        })
        .collect()
}
