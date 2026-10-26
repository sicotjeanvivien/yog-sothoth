//! Decoding of the fee parameters a DAMM v2 creation and fee update carry, which the
//! indexer keeps raw. ⚠️ Not a pool's state, read from its account (`application/decoder/`):
//! what one event set.

use thiserror::Error;

use crate::domain::{
    MeteoraDammV2BaseFee, MeteoraDammV2DynamicFee, MeteoraDammV2DynamicFeeUpdate,
    MeteoraDammV2FeeDecay, MeteoraDammV2PoolFees, MeteoraDammV2PoolFeesUpdate,
};

/// cp-amm's `BaseFeeParameters`: 27 bytes, whose last names their layout.
const BASE_FEE_LEN: usize = 27;
const BASE_FEE_MODE_OFFSET: usize = 26;
/// cp-amm's `DynamicFeeParameters`, borsh: 2 + 16 + 2 + 2 + 2 + 4 + 4.
const DYNAMIC_FEE_LEN: usize = 32;

/// Why a fee blob was not decoded: each reason its own metric label.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MeteoraDammV2FeeParametersRejection {
    #[error("unknown base fee mode {mode}")]
    UnknownBaseFeeMode { mode: u8 },
    #[error("`{field}`: Option tag {tag}, neither 0 nor 1")]
    InvalidOptionTag { field: &'static str, tag: u8 },
    #[error("`{field}` truncated: {len} bytes left, {need} needed")]
    Truncated {
        field: &'static str,
        len: usize,
        need: usize,
    },
    #[error("{len} bytes past the last field")]
    TrailingBytes { len: usize },
}

impl MeteoraDammV2FeeParametersRejection {
    /// The metric label of the reason.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownBaseFeeMode { .. } => "unknown_base_fee_mode",
            Self::InvalidOptionTag { .. } => "invalid_option_tag",
            Self::Truncated { .. } => "truncated",
            Self::TrailingBytes { .. } => "trailing_bytes",
        }
    }
}

type Rejection = MeteoraDammV2FeeParametersRejection;

/// The fees of a creation, from `pool_fees_raw`: cp-amm's `PoolFeeParameters`.
pub fn decode_meteora_damm_v2_pool_fees(raw: &[u8]) -> Result<MeteoraDammV2PoolFees, Rejection> {
    let mut reader = Reader::new(raw);
    let base_fee = decode_base_fee(reader.take::<BASE_FEE_LEN>("base_fee")?)?;
    let compounding_fee_bps = u16::from_le_bytes(reader.take("compounding_fee_bps")?);
    reader.take::<1>("padding")?;
    let dynamic_fee = if reader.option_tag("dynamic_fee")? {
        Some(decode_dynamic_fee(reader.take("dynamic_fee")?))
    } else {
        None
    };
    reader.finish()?;
    Ok(MeteoraDammV2PoolFees {
        base_fee,
        compounding_fee_bps,
        dynamic_fee,
    })
}

/// What a fee update changes, from `params_raw`: cp-amm's `UpdatePoolFeesParameters`.
pub fn decode_meteora_damm_v2_pool_fees_update(
    raw: &[u8],
) -> Result<MeteoraDammV2PoolFeesUpdate, Rejection> {
    let mut reader = Reader::new(raw);
    let cliff_fee_numerator = if reader.option_tag("cliff_fee_numerator")? {
        Some(u64::from_le_bytes(reader.take("cliff_fee_numerator")?))
    } else {
        None
    };
    let dynamic_fee = if !reader.option_tag("dynamic_fee")? {
        MeteoraDammV2DynamicFeeUpdate::Unchanged
    } else {
        let bytes = reader.take::<DYNAMIC_FEE_LEN>("dynamic_fee")?;
        if bytes.iter().all(|&b| b == 0) {
            MeteoraDammV2DynamicFeeUpdate::Disabled
        } else {
            MeteoraDammV2DynamicFeeUpdate::Set(decode_dynamic_fee(bytes))
        }
    };
    // ⚠️ Ending here is the program's earlier two-field layout, which had no compounding fee
    // to change: not a truncation, which this layout cannot tell apart.
    let compounding_fee_bps = if !reader.is_empty() && reader.option_tag("compounding_fee_bps")? {
        Some(u16::from_le_bytes(reader.take("compounding_fee_bps")?))
    } else {
        None
    };
    reader.finish()?;
    Ok(MeteoraDammV2PoolFeesUpdate {
        cliff_fee_numerator,
        dynamic_fee,
        compounding_fee_bps,
    })
}

fn decode_base_fee(bytes: [u8; BASE_FEE_LEN]) -> Result<MeteoraDammV2BaseFee, Rejection> {
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
    let cliff_fee_numerator = u64_at(0);
    let decay = |linear: u8, mode: u8| {
        if mode == linear {
            MeteoraDammV2FeeDecay::Linear
        } else {
            MeteoraDammV2FeeDecay::Exponential
        }
    };
    Ok(match bytes[BASE_FEE_MODE_OFFSET] {
        mode @ (0 | 1) => MeteoraDammV2BaseFee::TimeScheduler {
            decay: decay(0, mode),
            cliff_fee_numerator,
            number_of_period: u16_at(8),
            period_frequency: u64_at(10),
            reduction_factor: u64_at(18),
        },
        2 => MeteoraDammV2BaseFee::RateLimiter {
            cliff_fee_numerator,
            fee_increment_bps: u16_at(8),
            max_limiter_duration: u32_at(10),
            max_fee_bps: u32_at(14),
            reference_amount: u64_at(18),
        },
        mode @ (3 | 4) => MeteoraDammV2BaseFee::MarketCapScheduler {
            decay: decay(3, mode),
            cliff_fee_numerator,
            number_of_period: u16_at(8),
            sqrt_price_step_bps: u32_at(10),
            scheduler_expiration_duration: u32_at(14),
            reduction_factor: u64_at(18),
        },
        mode => return Err(Rejection::UnknownBaseFeeMode { mode }),
    })
}

fn decode_dynamic_fee(bytes: [u8; DYNAMIC_FEE_LEN]) -> MeteoraDammV2DynamicFee {
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    MeteoraDammV2DynamicFee {
        bin_step: u16_at(0),
        bin_step_u128: u128::from_le_bytes(bytes[2..18].try_into().expect("16 bytes")),
        filter_period: u16_at(18),
        decay_period: u16_at(20),
        reduction_factor: u16_at(22),
        max_volatility_accumulator: u32_at(24),
        variable_fee_control: u32_at(28),
    }
}

/// A borsh blob read field after field, each refusal naming its field.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { rest: raw }
    }

    fn take<const N: usize>(&mut self, field: &'static str) -> Result<[u8; N], Rejection> {
        let Some((head, rest)) = self.rest.split_first_chunk::<N>() else {
            return Err(Rejection::Truncated {
                field,
                len: self.rest.len(),
                need: N,
            });
        };
        self.rest = rest;
        Ok(*head)
    }

    /// A borsh `Option` tag: whether a value follows.
    fn option_tag(&mut self, field: &'static str) -> Result<bool, Rejection> {
        match self.take::<1>(field)? {
            [0] => Ok(false),
            [1] => Ok(true),
            [tag] => Err(Rejection::InvalidOptionTag { field, tag }),
        }
    }

    fn is_empty(&self) -> bool {
        self.rest.is_empty()
    }

    fn finish(self) -> Result<(), Rejection> {
        match self.rest.len() {
            0 => Ok(()),
            len => Err(Rejection::TrailingBytes { len }),
        }
    }
}

#[cfg(test)]
#[path = "fee_parameters_tests.rs"]
mod tests;
