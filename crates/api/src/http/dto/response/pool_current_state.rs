mod last_event_kind_response;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use super::pool::ProtocolResponse;
use crate::application::PoolCurrentStateView;
use crate::http::dto::schema::{U128String, UnsignedDecimal};

use last_event_kind_response::LastEventKindResponse;

/// The latest observed state of a pool. There is no liquidity `L` here: the
/// per-movement liquidity is on `/liquidity-events`, beside the kind that signs it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCurrentStateResponse {
    #[schema(min_length = 1)]
    pool_address: String,
    protocol: ProtocolResponse,

    last_event_at: DateTime<Utc>,
    last_event_kind: LastEventKindResponse,
    #[schema(min_length = 1)]
    last_signature: String,

    #[schema(value_type = U128String)]
    reserve_a: String,
    #[schema(value_type = U128String)]
    reserve_b: String,

    /// Q64.64 fixed-point; encoded as a string to keep precision in JS.
    #[schema(value_type = Option<U128String>)]
    last_sqrt_price: Option<String>,

    /// Units of token B per token A, decimal-adjusted, from `last_sqrt_price`: the
    /// concentrated-liquidity price, not the reserve ratio. `None` without a
    /// `sqrt_price` or with unresolved decimals.
    #[schema(value_type = Option<UnsignedDecimal>)]
    spot_price_a_in_b: Option<Decimal>,

    last_swap_at: Option<DateTime<Utc>>,

    updated_at: DateTime<Utc>,
}

impl From<PoolCurrentStateView> for PoolCurrentStateResponse {
    fn from(view: PoolCurrentStateView) -> Self {
        let PoolCurrentStateView {
            state,
            spot_price_a_in_b,
        } = view;
        Self {
            pool_address: state.pool_address.to_string(),
            protocol: state.protocol.into(),
            last_event_at: state.last_event_at,
            last_event_kind: state.last_event_kind.into(),
            last_signature: state.last_signature.to_string(),
            reserve_a: state.reserve_a.to_string(),
            reserve_b: state.reserve_b.to_string(),
            last_sqrt_price: state.last_sqrt_price.map(|v| v.to_string()),
            spot_price_a_in_b,
            last_swap_at: state.last_swap_at,
            updated_at: state.updated_at,
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_current_state_tests.rs"]
mod tests;
