use yog_core::{RepositoryError, domain::MeteoraDlmmPoolProperties};

use crate::repositories::helper::{
    convert_i16_to_u8, convert_i32_to_u16, convert_i64_to_u32, convert_string_to_pubkey,
};

/// Row shape returned by SELECTs on `meteora_dlmm_pool_properties`
/// (migration 039). Mirrors every column of the table.
///
/// Every column is nullable, and they are NULL **together**: all six come from
/// one read of one `LbPair` account, so a row is either fully resolved or a
/// placeholder for a pool the enrichment queue has not reached. Unlike cp-amm's
/// satellite there is no field with an independent failure mode.
///
/// The signed widths are one step up from the on-chain types — INTEGER for a
/// `u16`, BIGINT for a `u32` — because Postgres has no unsigned integers. See
/// migration 039.
pub(super) struct MeteoraDlmmPoolPropertiesRow {
    pub(super) pool_address: String,
    pub(super) bin_step: Option<i32>,
    pub(super) base_factor: Option<i32>,
    pub(super) base_fee_power_factor: Option<i16>,
    pub(super) variable_fee_control: Option<i64>,
    pub(super) max_volatility_accumulator: Option<i64>,
    pub(super) protocol_share: Option<i32>,
}

/// Lift a narrowing guard over `Option`. A corrupt row surfaces as `Integrity`
/// rather than being silently truncated — the same discipline as cp-amm's
/// `percent` helper.
fn u16_column(value: Option<i32>, field: &str) -> Result<Option<u16>, RepositoryError> {
    value.map(|v| convert_i32_to_u16(v, field)).transpose()
}

fn u32_column(value: Option<i64>, field: &str) -> Result<Option<u32>, RepositoryError> {
    value.map(|v| convert_i64_to_u32(v, field)).transpose()
}

impl TryFrom<MeteoraDlmmPoolPropertiesRow> for MeteoraDlmmPoolProperties {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDlmmPoolPropertiesRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDlmmPoolProperties {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            bin_step: u16_column(row.bin_step, "bin_step")?,
            base_factor: u16_column(row.base_factor, "base_factor")?,
            base_fee_power_factor: row
                .base_fee_power_factor
                .map(|v| convert_i16_to_u8(v, "base_fee_power_factor"))
                .transpose()?,
            variable_fee_control: u32_column(row.variable_fee_control, "variable_fee_control")?,
            max_volatility_accumulator: u32_column(
                row.max_volatility_accumulator,
                "max_volatility_accumulator",
            )?,
            protocol_share: u16_column(row.protocol_share, "protocol_share")?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
