//! Request DTO for `GET /api/protocols/{protocol}/tvl/by-token`.

use yog_core::domain::{Protocol, TokenTvlCursor, TokenTvlOrder};

use crate::http::{cursor::decode_token_tvl_cursor, error::ApiError};

mod sort_param;
mod token_tvl_params;

use sort_param::SortParam;
pub(crate) use token_tvl_params::TokenTvlParams;

#[derive(Debug)]
pub(crate) struct ListTokenTvlRequest {
    pub(crate) order: TokenTvlOrder,
    pub(crate) cursor: Option<TokenTvlCursor>,
    pub(crate) limit: i64,
}

impl ListTokenTvlRequest {
    pub(crate) fn parse(params: TokenTvlParams, protocol: Protocol) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let order = params.sort.into();
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_token_tvl_cursor(raw, protocol, order))
            })
            .transpose()?;
        Ok(Self {
            order,
            cursor,
            limit: params.limit,
        })
    }
}
