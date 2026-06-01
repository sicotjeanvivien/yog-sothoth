//! Request DTO for `GET /api/pools`.
//!
//! Aggregates every validation rule for the pool list endpoint:
//! pagination bounds, cursor + position mutual exclusivity, search
//! length, cursor ↔ sort consistency. By the time a
//! `ListPoolsRequest` is constructed, every field is guaranteed
//! valid; the handler can hand it straight to the service.

use yog_core::domain::PoolCursor;
use yog_core::{PageDirection, PagePosition, PoolSort};

use crate::application::PoolListParams;
use crate::http::{
    cursor::decode_pool_cursor,
    error::ApiError,
    query::{
        PageQuery, normalize_search, validate_cursor_sort_consistency, validate_limit,
        validate_pagination_query, validate_search,
    },
};

pub(crate) struct ListPoolsRequest {
    cursor: Option<PoolCursor>,
    direction: PageDirection,
    position: Option<PagePosition>,
    sort: PoolSort,
    search: Option<String>,
    limit: i64,
}

impl ListPoolsRequest {
    /// Run the full validation pipeline against a raw query payload.
    pub(crate) fn parse(query: PageQuery) -> Result<Self, ApiError> {
        validate_limit(query.limit)?;
        validate_pagination_query(&query)?;
        validate_search(query.q.as_deref())?;

        let cursor = match query.cursor.as_deref() {
            Some(raw) if !raw.is_empty() => Some(decode_pool_cursor(raw)?),
            _ => None,
        };
        let sort: PoolSort = query.sort.into();

        // Option B payoff: reject a cursor built for a different sort.
        validate_cursor_sort_consistency(cursor.as_ref(), sort)?;

        Ok(Self {
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            sort,
            search: normalize_search(query.q),
            limit: query.limit,
        })
    }

    /// Project into the service-layer params. Consumes self because
    /// the request DTO has no use past this point.
    pub(crate) fn into_params(self) -> PoolListParams {
        PoolListParams {
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            sort: self.sort,
            search: self.search,
            limit: self.limit,
        }
    }
}
