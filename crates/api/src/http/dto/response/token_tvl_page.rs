//! `/api/protocols/{protocol}/tvl/by-token`: a page of a protocol's tokens at its last TVL reading.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{TokenTvlPage, TokenTvlTotal};

use super::token_tvl::TokenTvlResponse;
use crate::http::cursor::encode_cursor_opt;
use crate::http::error::ApiError;

mod total_response;

use total_response::TokenTvlPageTotalResponse;

/// A forward-only page of the ranking at the reading `at`: `prevCursor` is always `null`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenTvlPageResponse {
    /// The close of the hour of the reading; `null` before the first.
    at: Option<DateTime<Utc>>,
    /// The first vesting observed: the locked parts leave out those locked before it.
    covered_since: Option<DateTime<Utc>>,
    total: TokenTvlPageTotalResponse,
    items: Vec<TokenTvlResponse>,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    is_first: bool,
    is_last: bool,
}

impl TokenTvlPageResponse {
    pub(crate) fn new(read: Option<TokenTvlPage>) -> Result<Self, ApiError> {
        let Some(read) = read else {
            return Ok(Self {
                at: None,
                covered_since: None,
                total: TokenTvlTotal {
                    tvl_usd: None,
                    locked_usd: None,
                }
                .into(),
                items: Vec::new(),
                next_cursor: None,
                prev_cursor: None,
                is_first: true,
                is_last: true,
            });
        };
        let page = read.page;
        Ok(Self {
            at: Some(read.at),
            covered_since: read.covered_since,
            total: read.total.into(),
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page.items.into_iter().map(Into::into).collect(),
        })
    }
}
