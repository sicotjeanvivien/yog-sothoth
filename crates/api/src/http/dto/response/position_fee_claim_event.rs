//! A fee claim of `/api/position-fee-claims` and its scoped lists: the position's fees taken out
//! of its pool, in raw units of each token. No field is one protocol's own.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PositionFeeClaimEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, U128String};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PositionFeeClaimEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    #[schema(min_length = 1)]
    position: String,
    #[schema(min_length = 1)]
    owner: String,
    #[schema(value_type = U128String)]
    fee_a: String,
    #[schema(value_type = U128String)]
    fee_b: String,
}

impl From<PositionFeeClaimEvent> for PositionFeeClaimEventResponse {
    fn from(event: PositionFeeClaimEvent) -> Self {
        let protocol = event.protocol();
        match event {
            PositionFeeClaimEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                position: e.position.to_string(),
                owner: e.owner.to_string(),
                fee_a: e.fee_a_claimed.to_string(),
                fee_b: e.fee_b_claimed.to_string(),
            },
        }
    }
}
