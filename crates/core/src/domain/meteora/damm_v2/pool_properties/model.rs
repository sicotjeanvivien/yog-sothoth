use solana_pubkey::Pubkey;

use super::FeeSchedulerDrop;
use crate::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams};

/// DAMM v2-only pool properties, the satellite of [`crate::domain::Pool`], as read.
/// Every field is `None` until yog-context resolves the account; after it,
/// `base_fee_kind` and `fee_scheduler` can still be `None` (see each).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2PoolProperties {
    pub pool_address: Pubkey,

    /// Meteora's cut of the trading fee, as a whole percent (0..=100).
    pub protocol_fee_percent: Option<u8>,

    /// A referrer's cut of the trading fee, as a whole percent (0..=100), charged
    /// only when a swap carries a referral account.
    pub referral_fee_percent: Option<u8>,

    /// A `BaseFeeKind::as_str`, `None` for a mode this build cannot map.
    pub base_fee_kind: Option<String>,

    /// Whether a volatility-based dynamic fee sits on top of the base fee.
    pub has_dynamic_fee: Option<bool>,

    /// The decay curve, to tell what the pool charges now; `None` also when the
    /// fee shape has no time curve (`base_fee_kind` tells which).
    pub fee_scheduler: Option<FeeSchedulerParams>,

    /// `Some` when a stored curve could not be rebuilt, so `fee_scheduler` hides it.
    pub fee_scheduler_dropped: Option<FeeSchedulerDrop>,
}

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

    /// The price range and the price at the read, Q64.64 square roots; these five are
    /// `None` together, for an account too short to hold them.
    /// ⚠️ A `Compounding` pool has no range: it reads `0` and `u128::MAX`.
    pub sqrt_min_price: Option<u128>,
    pub sqrt_max_price: Option<u128>,
    pub sqrt_price: Option<u128>,

    /// `CollectFeeMode`: 0 both tokens, 1 token B only, 2 compounding.
    pub collect_fee_mode: Option<u8>,

    /// The pool's liquidity locked for good, in L.
    pub permanent_lock_liquidity: Option<u128>,

    /// The unit of every point of the pool, a vesting's included: 0 slot, 1 timestamp.
    /// ⚠️ Not the scheduler's `activation_type`, which is kept only with a fee curve.
    pub pool_activation_type: Option<u8>,
}
