use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{PoolCurrentState, PoolValue};

use super::last_event_kind_response::PoolLastEventKindResponse;
use crate::http::dto::schema::{U128String, UnsignedDecimal};

/// A pool's state after the last event that moved it, and the value of its reserves now.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(in crate::http::dto::response) struct PoolStateResponse {
    last_event_at: DateTime<Utc>,
    last_event_kind: PoolLastEventKindResponse,
    /// In raw units.
    #[schema(value_type = U128String)]
    reserve_a: String,
    #[schema(value_type = U128String)]
    reserve_b: String,
    last_swap_at: Option<DateTime<Utc>>,
    /// `null` when token A's price is stale or unknown, or its decimals unresolved.
    #[schema(value_type = Option<UnsignedDecimal>)]
    value_a_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    value_b_usd: Option<Decimal>,
    /// The sides valued, summed; `null` when neither is.
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
}

impl PoolStateResponse {
    pub(in crate::http::dto::response) fn new(state: PoolCurrentState, value: PoolValue) -> Self {
        Self {
            last_event_at: state.last_event_at,
            last_event_kind: state.last_event_kind.into(),
            reserve_a: state.reserve_a.to_string(),
            reserve_b: state.reserve_b.to_string(),
            last_swap_at: state.last_swap_at,
            value_a_usd: value.value_a_usd,
            value_b_usd: value.value_b_usd,
            tvl_usd: value.tvl_usd,
        }
    }
}

#[cfg(test)]
#[path = "../tests/pool_state_tests.rs"]
mod tests;
