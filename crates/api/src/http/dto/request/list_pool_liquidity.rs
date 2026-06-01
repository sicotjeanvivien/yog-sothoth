//! Request DTO for `GET /api/pools/{address}/liquidity-events`.

use solana_pubkey::Pubkey;
use yog_core::domain::LiquidityCursor;
use yog_core::{PageDirection, PagePosition};

use crate::application::LiquidityListParams;
use crate::http::{
    cursor::decode_liquidity_cursor,
    error::ApiError,
    query::{PageQuery, parse_pool_address, validate_limit, validate_pagination_query},
};

pub(crate) struct ListPoolLiquidityRequest {
    pool_address: Pubkey,
    cursor: Option<LiquidityCursor>,
    direction: PageDirection,
    position: Option<PagePosition>,
    limit: i64,
}

impl ListPoolLiquidityRequest {
    pub(crate) fn parse(address: String, query: PageQuery) -> Result<Self, ApiError> {
        let pool_address = parse_pool_address(&address)?;
        validate_limit(query.limit)?;
        validate_pagination_query(&query)?;

        let cursor = match query.cursor.as_deref() {
            Some(raw) if !raw.is_empty() => Some(decode_liquidity_cursor(raw)?),
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

    pub(crate) fn into_params(self) -> LiquidityListParams {
        LiquidityListParams {
            pool_address: self.pool_address,
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            limit: self.limit,
        }
    }
}
