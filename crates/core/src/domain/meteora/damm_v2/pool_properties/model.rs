use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams};

/// DAMM v2-only pool properties, the satellite of [`crate::domain::Pool`].
/// The fields are `None` together until yog-context resolves the account; only
/// `base_fee_kind` can stay `None` alone, for a mode this build cannot map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDammV2PoolProperties {
    /// The pool these properties describe.
    pub pool_address: Pubkey,

    /// Meteora's cut of the trading fee, as a whole percent (0..=100).
    pub protocol_fee_percent: Option<u8>,

    /// A referrer's cut of the trading fee, as a whole percent (0..=100), charged
    /// only when a swap carries a referral account.
    pub referral_fee_percent: Option<u8>,

    /// How the base fee behaves over time: a `BaseFeeKind::as_str`.
    pub base_fee_kind: Option<String>,

    /// Whether a volatility-based dynamic fee sits on top of the base fee; a pool
    /// can run a scheduler and a dynamic fee at once.
    pub has_dynamic_fee: Option<bool>,

    /// The decay curve, to tell what the pool charges **now**; `None` also when the
    /// fee shape has no time curve (`base_fee_kind` tells which).
    pub fee_scheduler: Option<FeeSchedulerParams>,
}

/// What one read of a cp-amm `Pool` account yields for this protocol's satellite.
/// Total where [`MeteoraDammV2PoolProperties`] is partial: a read, not a row.
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
