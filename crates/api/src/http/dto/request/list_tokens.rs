//! Request DTO for `GET /api/tokens`.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::TokenCursor;

use crate::http::{cursor::decode_token_cursor, error::ApiError};

/// Forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct TokensParams {
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListTokensRequest {
    pub(crate) cursor: Option<TokenCursor>,
    pub(crate) limit: i64,
}

impl ListTokensRequest {
    pub(crate) fn parse(params: TokensParams) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| super::non_empty("cursor", raw).and_then(decode_token_cursor))
            .transpose()?;
        Ok(Self {
            cursor,
            limit: params.limit,
        })
    }
}
