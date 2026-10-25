//! A claim of `/api/protocol-fee-claims` and its scoped lists: the protocol's fees taken out of a
//! pool, in raw units of each token. No field is one protocol's own.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolFeeClaimEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, U128String};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolFeeClaimEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    #[schema(value_type = U128String)]
    amount_a: String,
    #[schema(value_type = U128String)]
    amount_b: String,
}

impl From<ProtocolFeeClaimEvent> for ProtocolFeeClaimEventResponse {
    fn from(event: ProtocolFeeClaimEvent) -> Self {
        let protocol = event.protocol();
        match event {
            ProtocolFeeClaimEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                amount_a: e.token_a_amount.to_string(),
                amount_b: e.token_b_amount.to_string(),
            },
        }
    }
}
