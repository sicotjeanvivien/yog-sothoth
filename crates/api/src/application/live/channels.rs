use std::sync::Arc;

use tokio::sync::broadcast;
use yog_core::domain::{
    LiquidityChangeEvent, LiquidityChangeTable, PoolChangeEvent, PoolChangeTable,
    PoolCreationEvent, PoolCreationTable, PositionChangeEvent, PositionChangeTable,
    PositionFeeClaimEvent, PositionFeeClaimTable, ProtocolFeeClaimEvent, ProtocolFeeClaimTable,
    RewardChangeEvent, RewardChangeTable, RewardClaimEvent, RewardClaimTable,
};

use super::LiveEvent;
use crate::application::{EnrichedSignal, Paired};

/// The broadcast of one event collection, written by the pollers of all its tables.
pub(crate) type EventChannel<T, E> = broadcast::Sender<Arc<LiveEvent<T, Paired<E>>>>;

/// One broadcast per topic of `/api/live`. ⚠️ A topic's pollers read only while its channel
/// has a receiver, so a connection subscribes only to the topics it names.
#[derive(Clone)]
pub(crate) struct LiveChannels {
    pub(crate) signals: broadcast::Sender<Arc<EnrichedSignal>>,
    pub(crate) liquidity_changes: EventChannel<LiquidityChangeTable, LiquidityChangeEvent>,
    pub(crate) pool_creations: EventChannel<PoolCreationTable, PoolCreationEvent>,
    pub(crate) position_changes: EventChannel<PositionChangeTable, PositionChangeEvent>,
    pub(crate) position_fee_claims: EventChannel<PositionFeeClaimTable, PositionFeeClaimEvent>,
    pub(crate) protocol_fee_claims: EventChannel<ProtocolFeeClaimTable, ProtocolFeeClaimEvent>,
    pub(crate) reward_claims: EventChannel<RewardClaimTable, RewardClaimEvent>,
    pub(crate) reward_changes: EventChannel<RewardChangeTable, RewardChangeEvent>,
    pub(crate) pool_changes: EventChannel<PoolChangeTable, PoolChangeEvent>,
}

#[cfg(test)]
impl LiveChannels {
    /// Channels no poller writes in.
    pub(crate) fn idle() -> Self {
        Self {
            signals: broadcast::channel(1).0,
            liquidity_changes: broadcast::channel(1).0,
            pool_creations: broadcast::channel(1).0,
            position_changes: broadcast::channel(1).0,
            position_fee_claims: broadcast::channel(1).0,
            protocol_fee_claims: broadcast::channel(1).0,
            reward_claims: broadcast::channel(1).0,
            reward_changes: broadcast::channel(1).0,
            pool_changes: broadcast::channel(1).0,
        }
    }
}
