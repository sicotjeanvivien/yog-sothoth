//! A claim of `/api/reward-claims` and its scoped lists: a position's reward taken out of its pool,
//! in raw units of the reward's token. No field is one protocol's own.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::RewardClaimEvent;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, U128String};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RewardClaimEventResponse {
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
    #[schema(min_length = 1)]
    reward_mint: String,
    reward_index: u8,
    #[schema(value_type = U128String)]
    amount: String,
}

impl From<RewardClaimEvent> for RewardClaimEventResponse {
    fn from(event: RewardClaimEvent) -> Self {
        let protocol = event.protocol();
        match event {
            RewardClaimEvent::MeteoraDammV2(e) => Self {
                protocol: protocol.into(),
                pool_address: e.pool_address.to_string(),
                signature: e.signature.to_string(),
                event_index: e.event_index,
                slot: e.slot,
                at: e.timestamp,
                position: e.position.to_string(),
                owner: e.owner.to_string(),
                reward_mint: e.mint_reward.to_string(),
                reward_index: e.reward_index,
                amount: e.total_reward.to_string(),
            },
        }
    }
}

#[cfg(test)]
#[path = "tests/reward_claim_event_tests.rs"]
mod tests;
