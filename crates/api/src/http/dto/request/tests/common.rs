//! Valid baselines each request DTO test starts from, mutating only what it tests.

use crate::http::query::{EventPageQuery, PageDirectionParam, PageQuery, PoolSortParam};

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

/// A valid `EventPageQuery` baseline: the same, without sort or search.
pub(crate) fn valid_event_page_query() -> EventPageQuery {
    EventPageQuery {
        cursor: None,
        dir: PageDirectionParam::Next,
        position: None,
        limit: 50,
    }
}
