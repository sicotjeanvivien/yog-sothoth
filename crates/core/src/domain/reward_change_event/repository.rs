use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::RewardChangeEvent;
use crate::domain::{EventCursor, EventScope};
use crate::{Page, RepositoryResult};

#[async_trait]
pub trait RewardChangeEventLookup: Send + Sync {
    /// A page of `scope`'s changes from `from` on, every kind merged, the most recent first, ties
    /// by signature then event index descending, after `cursor`'s key; forward only.
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RewardChangeEvent>>;
}

crate::domain::event_tables! {
    /// The tables the reward changes are written in, one per protocol and event kind.
    RewardChangeTable {
        MeteoraDammV2Initialize => (MeteoraDammV2, "meteora_damm_v2_initialize_reward_events"),
        MeteoraDammV2Fund => (MeteoraDammV2, "meteora_damm_v2_fund_reward_events"),
        MeteoraDammV2DurationChange => (MeteoraDammV2, "meteora_damm_v2_update_reward_duration_events"),
        MeteoraDammV2FunderChange => (MeteoraDammV2, "meteora_damm_v2_update_reward_funder_events"),
        MeteoraDammV2IneligibleWithdrawal => (MeteoraDammV2, "meteora_damm_v2_withdraw_ineligible_reward_events"),
        MeteoraDammV2DeadLiquidityWithdrawal => (MeteoraDammV2, "meteora_damm_v2_withdraw_dead_liquidity_reward_events"),
    }
}
