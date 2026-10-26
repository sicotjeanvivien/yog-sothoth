//! The fee parameters a DAMM v2 pool is created with, and what an operator's fee update
//! changes, as cp-amm writes them. Fees are numerators over 1e9; a period is a slot or a
//! second, as the pool's `activation_type` says.

/// How a scheduled fee falls from its cliff, period after period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteoraDammV2FeeDecay {
    Linear,
    Exponential,
}

/// The base fee, one variant per layout of cp-amm's `BaseFeeParameters`.
/// ⚠️ A scheduler with `number_of_period == 0` is a constant fee: see `base_fee_kind_from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteoraDammV2BaseFee {
    /// Modes 0 and 1: the fee falls with the time since activation.
    TimeScheduler {
        decay: MeteoraDammV2FeeDecay,
        cliff_fee_numerator: u64,
        number_of_period: u16,
        period_frequency: u64,
        reduction_factor: u64,
    },
    /// Mode 2, deprecated by cp-amm: the fee rises with the size of a swap.
    RateLimiter {
        cliff_fee_numerator: u64,
        fee_increment_bps: u16,
        max_limiter_duration: u32,
        max_fee_bps: u32,
        reference_amount: u64,
    },
    /// Modes 3 and 4: the fee falls as the price rises from its first value.
    MarketCapScheduler {
        decay: MeteoraDammV2FeeDecay,
        cliff_fee_numerator: u64,
        number_of_period: u16,
        sqrt_price_step_bps: u32,
        scheduler_expiration_duration: u32,
        reduction_factor: u64,
    },
}

/// cp-amm's `DynamicFeeParameters`: the volatility fee added on top of the base fee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteoraDammV2DynamicFee {
    pub bin_step: u16,
    pub bin_step_u128: u128,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub max_volatility_accumulator: u32,
    pub variable_fee_control: u32,
}

/// The fees of a pool at its creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteoraDammV2PoolFees {
    pub base_fee: MeteoraDammV2BaseFee,
    pub compounding_fee_bps: u16,
    /// `None`: the pool has no dynamic fee.
    pub dynamic_fee: Option<MeteoraDammV2DynamicFee>,
}

/// What a fee update does to the dynamic fee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteoraDammV2DynamicFeeUpdate {
    Unchanged,
    /// cp-amm reads parameters at their default, all zero, as "turn it off".
    Disabled,
    /// Turned on, or its parameters replaced.
    Set(MeteoraDammV2DynamicFee),
}

/// What an operator's fee update changes; `None` leaves the field as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteoraDammV2PoolFeesUpdate {
    pub cliff_fee_numerator: Option<u64>,
    pub dynamic_fee: MeteoraDammV2DynamicFeeUpdate,
    pub compounding_fee_bps: Option<u16>,
}
