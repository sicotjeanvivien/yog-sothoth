use yog_core::{RepositoryError, domain::MeteoraDlmmPoolProperties};

use crate::repositories::helper::{
    convert_i16_to_u8, convert_i32_to_u16, convert_i64_to_u32, convert_optional,
    convert_string_to_pubkey,
};

/// Row shape of `meteora_dlmm_pool_properties` (baseline §9). The columns are NULL
/// together, and each is one signed width above its on-chain type.
pub(super) struct MeteoraDlmmPoolPropertiesRow {
    pub(super) pool_address: String,
    pub(super) bin_step: Option<i32>,
    pub(super) base_factor: Option<i32>,
    pub(super) base_fee_power_factor: Option<i16>,
    pub(super) variable_fee_control: Option<i64>,
    pub(super) max_volatility_accumulator: Option<i64>,
    pub(super) protocol_share: Option<i32>,
}

/// A value outside its on-chain type surfaces as `Integrity`, not truncated.
impl TryFrom<MeteoraDlmmPoolPropertiesRow> for MeteoraDlmmPoolProperties {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDlmmPoolPropertiesRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDlmmPoolProperties {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            bin_step: convert_optional(row.bin_step, "bin_step", convert_i32_to_u16)?,
            base_factor: convert_optional(row.base_factor, "base_factor", convert_i32_to_u16)?,
            base_fee_power_factor: convert_optional(
                row.base_fee_power_factor,
                "base_fee_power_factor",
                convert_i16_to_u8,
            )?,
            variable_fee_control: convert_optional(
                row.variable_fee_control,
                "variable_fee_control",
                convert_i64_to_u32,
            )?,
            max_volatility_accumulator: convert_optional(
                row.max_volatility_accumulator,
                "max_volatility_accumulator",
                convert_i64_to_u32,
            )?,
            protocol_share: convert_optional(
                row.protocol_share,
                "protocol_share",
                convert_i32_to_u16,
            )?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
