use crate::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams};

/// What one read of a cp-amm `Pool` account yields for this protocol's satellite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2PoolAccountProperties {
    /// Fee-split percents (0..=100): Meteora's cut and a referrer's cut.
    pub protocol_fee_percent: u8,
    pub referral_fee_percent: u8,

    /// How the base fee behaves over time.
    /// ⚠️ Optional so an unknown mode costs this field only: refusing the account
    /// would strand the pool at the head of the unresolved queue.
    pub base_fee_kind: Option<BaseFeeKind>,

    /// Whether a volatility-based dynamic fee is enabled.
    pub has_dynamic_fee: bool,

    /// The decay curve, `Some` **only** for the two time-scheduler modes.
    pub fee_scheduler: Option<FeeSchedulerParams>,
}
