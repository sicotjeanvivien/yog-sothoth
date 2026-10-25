//! A position change of `/api/position-changes` and its scoped lists: what every protocol's
//! change says, then the change itself under the protocol's name, tagged by its `kind`.

mod meteora_damm_v2_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PositionChangeEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

use meteora_damm_v2_response::PositionChangeEventMeteoraDammV2Response;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PositionChangeEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<PositionChangeEventMeteoraDammV2Response>,
}

impl From<PositionChangeEvent> for PositionChangeEventResponse {
    fn from(event: PositionChangeEvent) -> Self {
        let protocol = event.protocol();
        match event {
            PositionChangeEvent::MeteoraDammV2(change) => {
                let (at, signature, event_index) = change.key();
                Self {
                    protocol: protocol.into(),
                    pool_address: change.pool_address().to_string(),
                    signature: signature.to_string(),
                    event_index,
                    slot: change.slot(),
                    at,
                    meteora_damm_v2: Some(change.into()),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/position_change_event_tests.rs"]
mod tests;
