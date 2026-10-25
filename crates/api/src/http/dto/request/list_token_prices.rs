//! Request DTO for `GET /api/tokens/{mint}/prices`.

use serde::Deserialize;
use solana_pubkey::Pubkey;
use utoipa::IntoParams;
use yog_core::domain::{TokenPriceCursor, Window};

use super::window_param::WindowParam;
use crate::http::{cursor::decode_token_price_cursor, error::ApiError};

/// A window, ending at the first page, and forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct TokenPricesParams {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListTokenPricesRequest {
    pub(crate) mint: Pubkey,
    pub(crate) window: Window,
    pub(crate) cursor: Option<TokenPriceCursor>,
    pub(crate) limit: i64,
}

impl ListTokenPricesRequest {
    pub(crate) fn parse(mint: &str, params: TokenPricesParams) -> Result<Self, ApiError> {
        let mint = super::parse_address(mint, "mint")?;
        super::validate_limit(params.limit)?;
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw).and_then(|raw| decode_token_price_cursor(raw, mint))
            })
            .transpose()?;
        Ok(Self {
            mint,
            window: params.window.into(),
            cursor,
            limit: params.limit,
        })
    }
}
