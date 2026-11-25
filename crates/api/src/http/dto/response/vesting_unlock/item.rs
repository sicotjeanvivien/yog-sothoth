//! An item of `/api/protocols/{protocol}/vesting-unlocks`: one position's release at one instant.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::VestingUnlock;

use crate::http::dto::response::protocol::ProtocolResponse;
use crate::http::dto::schema::{U128String, UnsignedDecimal};

/// What one position's vesting schedules release at one instant.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VestingUnlockResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    /// `null` before its metadata is fetched.
    token_a_symbol: Option<String>,
    token_b_symbol: Option<String>,
    #[schema(min_length = 1)]
    position: String,
    unlocks_at: DateTime<Utc>,
    /// In L, the pool's unit of liquidity.
    #[schema(value_type = U128String)]
    liquidity: String,
    /// At the prices of the reading; `null` when its pool is not valued.
    #[schema(value_type = Option<UnsignedDecimal>)]
    amount_usd: Option<Decimal>,
}

impl From<VestingUnlock> for VestingUnlockResponse {
    fn from(u: VestingUnlock) -> Self {
        Self {
            protocol: u.protocol.into(),
            pool_address: u.pool_address.to_string(),
            token_a_symbol: u.token_a_symbol,
            token_b_symbol: u.token_b_symbol,
            position: u.position.to_string(),
            unlocks_at: u.unlocks_at,
            liquidity: u.liquidity.to_string(),
            amount_usd: u.amount_usd,
        }
    }
}
