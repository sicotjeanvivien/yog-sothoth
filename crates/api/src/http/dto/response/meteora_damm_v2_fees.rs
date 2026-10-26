//! The fee parameters of a DAMM v2 pool creation and of a fee update, as cp-amm writes them: a
//! fee is a numerator over 1e9, a scheduler's period a slot or a second as the creation's
//! `activationType` says. Each state is named, never `null`, which says the blob is unread.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    MeteoraDammV2BaseFee, MeteoraDammV2DynamicFee, MeteoraDammV2DynamicFeeUpdate,
    MeteoraDammV2FeeDecay, MeteoraDammV2PoolFees, MeteoraDammV2PoolFeesUpdate,
};

use crate::http::dto::schema::U128String;

/// The fees a pool is created with.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2FeeParametersResponse {
    base_fee: MeteoraDammV2BaseFeeResponse,
    compounding_fee_bps: u16,
    dynamic_fee: MeteoraDammV2DynamicFeeStateResponse,
}

/// The base fee, named by the `mode` cp-amm writes; a scheduler of 0 periods never moves.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum MeteoraDammV2BaseFeeResponse {
    TimeSchedulerLinear(MeteoraDammV2TimeSchedulerResponse),
    TimeSchedulerExponential(MeteoraDammV2TimeSchedulerResponse),
    RateLimiter(MeteoraDammV2RateLimiterResponse),
    MarketCapSchedulerLinear(MeteoraDammV2MarketCapSchedulerResponse),
    MarketCapSchedulerExponential(MeteoraDammV2MarketCapSchedulerResponse),
}

/// A fee that falls with the time since activation.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MeteoraDammV2TimeSchedulerResponse {
    #[schema(value_type = U128String)]
    cliff_fee_numerator: String,
    number_of_period: u16,
    #[schema(value_type = U128String)]
    period_frequency: String,
    #[schema(value_type = U128String)]
    reduction_factor: String,
}

/// A fee that rises with the size of a swap.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MeteoraDammV2RateLimiterResponse {
    #[schema(value_type = U128String)]
    cliff_fee_numerator: String,
    fee_increment_bps: u16,
    max_limiter_duration: u32,
    max_fee_bps: u32,
    #[schema(value_type = U128String)]
    reference_amount: String,
}

/// A fee that falls as the price rises from its first value.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MeteoraDammV2MarketCapSchedulerResponse {
    #[schema(value_type = U128String)]
    cliff_fee_numerator: String,
    number_of_period: u16,
    sqrt_price_step_bps: u32,
    scheduler_expiration_duration: u32,
    #[schema(value_type = U128String)]
    reduction_factor: String,
}

/// Whether the pool is created with a dynamic fee.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
enum MeteoraDammV2DynamicFeeStateResponse {
    Off,
    On(MeteoraDammV2DynamicFeeResponse),
}

/// The volatility fee added on top of the base fee.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MeteoraDammV2DynamicFeeResponse {
    bin_step: u16,
    #[schema(value_type = U128String)]
    bin_step_u128: String,
    filter_period: u16,
    decay_period: u16,
    reduction_factor: u16,
    max_volatility_accumulator: u32,
    variable_fee_control: u32,
}

/// What a fee update changes, field by field.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2FeeChangesResponse {
    cliff_fee_numerator: MeteoraDammV2NumeratorChangeResponse,
    dynamic_fee: MeteoraDammV2DynamicFeeChangeResponse,
    compounding_fee_bps: MeteoraDammV2BpsChangeResponse,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "change", rename_all = "snake_case")]
enum MeteoraDammV2NumeratorChangeResponse {
    Unchanged,
    Set {
        #[schema(value_type = U128String)]
        value: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "change", rename_all = "snake_case")]
enum MeteoraDammV2BpsChangeResponse {
    Unchanged,
    Set { value: u16 },
}

/// `disabled`: cp-amm reads parameters all at zero as "turn the dynamic fee off".
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "change", rename_all = "snake_case")]
enum MeteoraDammV2DynamicFeeChangeResponse {
    Unchanged,
    Disabled,
    Set(MeteoraDammV2DynamicFeeResponse),
}

impl From<MeteoraDammV2PoolFees> for MeteoraDammV2FeeParametersResponse {
    fn from(fees: MeteoraDammV2PoolFees) -> Self {
        Self {
            base_fee: fees.base_fee.into(),
            compounding_fee_bps: fees.compounding_fee_bps,
            dynamic_fee: match fees.dynamic_fee {
                Some(dynamic) => MeteoraDammV2DynamicFeeStateResponse::On(dynamic.into()),
                None => MeteoraDammV2DynamicFeeStateResponse::Off,
            },
        }
    }
}

impl From<MeteoraDammV2BaseFee> for MeteoraDammV2BaseFeeResponse {
    fn from(base: MeteoraDammV2BaseFee) -> Self {
        use MeteoraDammV2FeeDecay::{Exponential, Linear};
        match base {
            MeteoraDammV2BaseFee::TimeScheduler {
                decay,
                cliff_fee_numerator,
                number_of_period,
                period_frequency,
                reduction_factor,
            } => {
                let scheduler = MeteoraDammV2TimeSchedulerResponse {
                    cliff_fee_numerator: cliff_fee_numerator.to_string(),
                    number_of_period,
                    period_frequency: period_frequency.to_string(),
                    reduction_factor: reduction_factor.to_string(),
                };
                match decay {
                    Linear => Self::TimeSchedulerLinear(scheduler),
                    Exponential => Self::TimeSchedulerExponential(scheduler),
                }
            }
            MeteoraDammV2BaseFee::RateLimiter {
                cliff_fee_numerator,
                fee_increment_bps,
                max_limiter_duration,
                max_fee_bps,
                reference_amount,
            } => Self::RateLimiter(MeteoraDammV2RateLimiterResponse {
                cliff_fee_numerator: cliff_fee_numerator.to_string(),
                fee_increment_bps,
                max_limiter_duration,
                max_fee_bps,
                reference_amount: reference_amount.to_string(),
            }),
            MeteoraDammV2BaseFee::MarketCapScheduler {
                decay,
                cliff_fee_numerator,
                number_of_period,
                sqrt_price_step_bps,
                scheduler_expiration_duration,
                reduction_factor,
            } => {
                let scheduler = MeteoraDammV2MarketCapSchedulerResponse {
                    cliff_fee_numerator: cliff_fee_numerator.to_string(),
                    number_of_period,
                    sqrt_price_step_bps,
                    scheduler_expiration_duration,
                    reduction_factor: reduction_factor.to_string(),
                };
                match decay {
                    Linear => Self::MarketCapSchedulerLinear(scheduler),
                    Exponential => Self::MarketCapSchedulerExponential(scheduler),
                }
            }
        }
    }
}

impl From<MeteoraDammV2DynamicFee> for MeteoraDammV2DynamicFeeResponse {
    fn from(dynamic: MeteoraDammV2DynamicFee) -> Self {
        Self {
            bin_step: dynamic.bin_step,
            bin_step_u128: dynamic.bin_step_u128.to_string(),
            filter_period: dynamic.filter_period,
            decay_period: dynamic.decay_period,
            reduction_factor: dynamic.reduction_factor,
            max_volatility_accumulator: dynamic.max_volatility_accumulator,
            variable_fee_control: dynamic.variable_fee_control,
        }
    }
}

impl From<MeteoraDammV2PoolFeesUpdate> for MeteoraDammV2FeeChangesResponse {
    fn from(update: MeteoraDammV2PoolFeesUpdate) -> Self {
        Self {
            cliff_fee_numerator: match update.cliff_fee_numerator {
                Some(value) => MeteoraDammV2NumeratorChangeResponse::Set {
                    value: value.to_string(),
                },
                None => MeteoraDammV2NumeratorChangeResponse::Unchanged,
            },
            dynamic_fee: match update.dynamic_fee {
                MeteoraDammV2DynamicFeeUpdate::Unchanged => {
                    MeteoraDammV2DynamicFeeChangeResponse::Unchanged
                }
                MeteoraDammV2DynamicFeeUpdate::Disabled => {
                    MeteoraDammV2DynamicFeeChangeResponse::Disabled
                }
                MeteoraDammV2DynamicFeeUpdate::Set(dynamic) => {
                    MeteoraDammV2DynamicFeeChangeResponse::Set(dynamic.into())
                }
            },
            compounding_fee_bps: match update.compounding_fee_bps {
                Some(value) => MeteoraDammV2BpsChangeResponse::Set { value },
                None => MeteoraDammV2BpsChangeResponse::Unchanged,
            },
        }
    }
}
