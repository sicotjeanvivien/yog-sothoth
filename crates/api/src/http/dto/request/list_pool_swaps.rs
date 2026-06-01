//! Request DTO for `GET /api/pools/{address}/swaps`.

use solana_pubkey::Pubkey;
use yog_core::domain::SwapCursor;
use yog_core::{PageDirection, PagePosition};

use crate::application::SwapListParams;
use crate::http::{
    cursor::decode_swap_cursor,
    error::ApiError,
    query::{PageQuery, parse_pool_address, validate_limit, validate_pagination_query},
};

#[derive(Debug)]
pub(crate) struct ListPoolSwapsRequest {
    pool_address: Pubkey,
    cursor: Option<SwapCursor>,
    direction: PageDirection,
    position: Option<PagePosition>,
    limit: i64,
}

impl ListPoolSwapsRequest {
    /// Combine path and query extractors into a single validated request.
    ///
    /// Search and sort are intentionally not exposed by this endpoint —
    /// swap events are ordered by `(timestamp DESC, signature ASC)` by
    /// contract.
    pub(crate) fn parse(address: String, query: PageQuery) -> Result<Self, ApiError> {
        let pool_address = parse_pool_address(&address)?;
        validate_limit(query.limit)?;
        validate_pagination_query(&query)?;

        let cursor = match query.cursor.as_deref() {
            Some(raw) if !raw.is_empty() => Some(decode_swap_cursor(raw)?),
            _ => None,
        };

        Ok(Self {
            pool_address,
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
    }

    pub(crate) fn into_params(self) -> SwapListParams {
        SwapListParams {
            pool_address: self.pool_address,
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            limit: self.limit,
        }
    }
}

#[cfg(test)]
#[path = "tests/list_pool_swaps_tests.rs"]
mod tests;
