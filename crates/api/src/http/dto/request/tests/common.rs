//! Test helpers for request DTO tests.
//!
//! Provides a `valid_page_query()` constructor — every test starts
//! from a known-valid baseline and mutates only the fields it cares
//! about. Avoids 8-line boilerplate per test and keeps the intent
//! visible.

use crate::http::query::{PageDirectionParam, PageQuery, PoolSortParam};

/// A valid `PageQuery` baseline: no cursor, default direction, no
/// position, default sort, no search, default limit.
pub(crate) fn valid_page_query() -> PageQuery {
    PageQuery {
        cursor: None,
        dir: PageDirectionParam::Next,
        position: None,
        sort: PoolSortParam::default(),
        q: None,
        fee_bps: None,
        limit: 50,
    }
}
