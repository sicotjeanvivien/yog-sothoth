use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::domain::{ClaimPositionFeeEvent, ClaimRewardEvent, LiquidityEvent, Protocol, SwapEvent};

/// Sum type of every domain-level event Yog-Sothoth can extract from a
/// transaction.
///
/// Produced by the protocol-specific extractors (DAMM v2, …) as the result
/// of translating their wire events into protocol-agnostic domain events.
/// Consumed by the indexer service, which dispatches each variant to the
/// matching repository.
///
/// New event categories (cercle 2 / 3) will add variants here.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    Swap(SwapEvent),
    Liquidity(LiquidityEvent),
    ClaimPositionFee(ClaimPositionFeeEvent),
    ClaimReward(ClaimRewardEvent),
}

impl DomainEvent {
    /// Pool the event refers to.
    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::Swap(e) => e.pool_address,
            Self::Liquidity(e) => e.pool_address,
            Self::ClaimPositionFee(e) => e.pool_address,
            Self::ClaimReward(e) => e.pool_address,
        }
    }

    /// Transaction signature this event came from.
    pub fn signature(&self) -> &str {
        match self {
            Self::Swap(e) => &e.signature,
            Self::Liquidity(e) => &e.signature,
            Self::ClaimPositionFee(e) => &e.signature,
            Self::ClaimReward(e) => &e.signature,
        }
    }

    /// Block timestamp at which the source transaction was confirmed.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Swap(e) => e.timestamp,
            Self::Liquidity(e) => e.timestamp,
            Self::ClaimPositionFee(e) => e.timestamp,
            Self::ClaimReward(e) => e.timestamp,
        }
    }

    /// Protocol that emitted the event.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::Swap(e) => e.protocol,
            Self::Liquidity(e) => e.protocol,
            Self::ClaimPositionFee(e) => e.protocol,
            Self::ClaimReward(e) => e.protocol,
        }
    }

    /// Stable kind label, suitable for metrics and structured logs.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Swap(_) => "swap",
            Self::Liquidity(_) => "liquidity",
            Self::ClaimPositionFee(_) => "claim_position_fee",
            Self::ClaimReward(_) => "claim_reward",
        }
    }
}
