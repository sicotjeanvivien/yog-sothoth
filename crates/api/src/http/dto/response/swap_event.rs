//! A swap of `/api/swaps` and its scoped lists: what every protocol's swap says, then its own
//! protocol's fields under the protocol's name. Amounts and reserves in raw units.

mod direction_response;
mod fee_token_response;
mod meteora_damm_v2_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::SwapEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, U128String};

use direction_response::SwapEventDirectionResponse;
use meteora_damm_v2_response::SwapEventMeteoraDammV2Response;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    direction: SwapEventDirectionResponse,
    #[schema(value_type = U128String)]
    amount_a: String,
    #[schema(value_type = U128String)]
    amount_b: String,
    #[schema(value_type = U128String)]
    reserve_a_after: String,
    #[schema(value_type = U128String)]
    reserve_b_after: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<SwapEventMeteoraDammV2Response>,
}

impl From<SwapEvent> for SwapEventResponse {
    fn from(event: SwapEvent) -> Self {
        let protocol = event.protocol();
        match event {
            SwapEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                direction: e.trade_direction.into(),
                amount_a: e.amount_a.to_string(),
                amount_b: e.amount_b.to_string(),
                reserve_a_after: e.reserve_a_after.to_string(),
                reserve_b_after: e.reserve_b_after.to_string(),
                meteora_damm_v2: Some(SwapEventMeteoraDammV2Response::from(&e)),
            },
        }
    }
}
