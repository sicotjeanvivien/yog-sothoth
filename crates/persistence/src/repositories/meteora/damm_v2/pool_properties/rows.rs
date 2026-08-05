use yog_core::{RepositoryError, domain::MeteoraDammV2PoolProperties};

use crate::repositories::helper::{convert_i16_to_u8, convert_optional, convert_string_to_pubkey};

/// Row shape returned by SELECTs on `meteora_damm_v2_pool_properties`
/// (baseline §8). Mirrors every column of the table.
///
/// Every column is nullable because the two groups have different writers and
/// either can land first: the fee-split percents come from yog-context reading
/// the on-chain account, the fee shape from the indexer decoding the genesis
/// event.
pub(super) struct MeteoraDammV2PoolPropertiesRow {
    pub(super) pool_address: String,
    pub(super) protocol_fee_percent: Option<i16>,
    pub(super) referral_fee_percent: Option<i16>,
    pub(super) base_fee_kind: Option<String>,
    pub(super) has_dynamic_fee: Option<bool>,
}

/// The percent columns only ever hold values written from a `u8` (0..=100), but
/// a corrupt row surfaces as `Integrity` rather than being silently truncated —
/// the guard is `convert_i16_to_u8`, lifted over `Option` by `convert_optional`.
impl TryFrom<MeteoraDammV2PoolPropertiesRow> for MeteoraDammV2PoolProperties {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2PoolPropertiesRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2PoolProperties {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol_fee_percent: convert_optional(
                row.protocol_fee_percent,
                "protocol_fee_percent",
                convert_i16_to_u8,
            )?,
            referral_fee_percent: convert_optional(
                row.referral_fee_percent,
                "referral_fee_percent",
                convert_i16_to_u8,
            )?,
            base_fee_kind: row.base_fee_kind,
            has_dynamic_fee: row.has_dynamic_fee,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
