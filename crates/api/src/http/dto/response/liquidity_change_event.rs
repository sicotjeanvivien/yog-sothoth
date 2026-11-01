//! A liquidity change of `/api/liquidity-changes` and its scoped lists: what every protocol's
//! change says, then its own protocol's fields under the protocol's name. Amounts and
//! reserves in raw units.

mod kind_response;
mod meteora_damm_v2_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::LiquidityChangeEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, U128String};

use kind_response::LiquidityChangeEventKindResponse;
use meteora_damm_v2_response::LiquidityChangeEventMeteoraDammV2Response;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityChangeEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    kind: LiquidityChangeEventKindResponse,
    #[schema(value_type = U128String)]
    amount_a: String,
    #[schema(value_type = U128String)]
    amount_b: String,
    #[schema(value_type = U128String)]
    reserve_a_after: String,
    #[schema(value_type = U128String)]
    reserve_b_after: String,
    #[schema(min_length = 1)]
    position: String,
    #[schema(min_length = 1)]
    owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<LiquidityChangeEventMeteoraDammV2Response>,
}

impl From<LiquidityChangeEvent> for LiquidityChangeEventResponse {
    fn from(event: LiquidityChangeEvent) -> Self {
        let protocol = event.protocol();
        match event {
            LiquidityChangeEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                kind: e.liquidity_event_kind.into(),
                amount_a: e.amount_a.to_string(),
                amount_b: e.amount_b.to_string(),
                reserve_a_after: e.reserve_a_after.to_string(),
                reserve_b_after: e.reserve_b_after.to_string(),
                position: e.position.to_string(),
                owner: e.owner.to_string(),
                meteora_damm_v2: Some(LiquidityChangeEventMeteoraDammV2Response {
                    liquidity_delta: e.liquidity_delta.to_string(),
                }),
            },
        }
    }
}

#[cfg(test)]
#[path = "tests/liquidity_change_event_tests.rs"]
mod tests;
