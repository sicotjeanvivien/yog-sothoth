//! A pool creation of `/api/pool-creations` and its protocol's list: what every protocol's
//! creation says, then its own protocol's fields under the protocol's name.

mod meteora_damm_v2_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolCreationEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

use meteora_damm_v2_response::PoolCreationEventMeteoraDammV2Response;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCreationEventResponse {
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
    token_a_mint: String,
    #[schema(min_length = 1)]
    token_b_mint: String,
    #[schema(min_length = 1)]
    creator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<PoolCreationEventMeteoraDammV2Response>,
}

impl From<PoolCreationEvent> for PoolCreationEventResponse {
    fn from(event: PoolCreationEvent) -> Self {
        let protocol = event.protocol();
        match event {
            PoolCreationEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                token_a_mint: e.token_a_mint.to_string(),
                token_b_mint: e.token_b_mint.to_string(),
                creator: e.creator.to_string(),
                meteora_damm_v2: Some(PoolCreationEventMeteoraDammV2Response::from(&e)),
            },
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_creation_event_tests.rs"]
mod tests;
