//! A pool change of `/api/pool-changes` and its scoped lists: what every protocol's change says,
//! then the change itself under the protocol's name, tagged by its `kind`.

mod meteora_damm_v2_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolChangeEvent;

use super::protocol::ProtocolResponse;
use super::token::EmbeddedTokenResponse;
use crate::application::Paired;
use crate::http::dto::schema::Count;

use meteora_damm_v2_response::PoolChangeEventMeteoraDammV2Response;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolChangeEventResponse {
    protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pool_address: String,
    /// The pool's token pair: every field nullable until yog-context resolves the pool.
    token_a: EmbeddedTokenResponse,
    token_b: EmbeddedTokenResponse,
    #[schema(min_length = 1)]
    signature: String,
    event_index: u16,
    #[schema(value_type = Count)]
    slot: u64,
    at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<PoolChangeEventMeteoraDammV2Response>,
}

impl From<Paired<PoolChangeEvent>> for PoolChangeEventResponse {
    fn from(paired: Paired<PoolChangeEvent>) -> Self {
        let Paired {
            item: event,
            token_a,
            token_b,
        } = paired;
        let protocol = event.protocol();
        match event {
            PoolChangeEvent::MeteoraDammV2(change) => {
                let (at, signature, event_index) = change.key();
                Self {
                    protocol: protocol.into(),
                    pool_address: change.pool_address().to_string(),
                    token_a: token_a.into(),
                    token_b: token_b.into(),
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
#[path = "tests/pool_change_event_tests.rs"]
mod tests;
