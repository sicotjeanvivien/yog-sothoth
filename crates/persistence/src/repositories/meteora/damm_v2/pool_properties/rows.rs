use yog_core::{
    RepositoryError,
    amm::damm_v2::{BaseFeeKind, FeeSchedulerParams},
    domain::MeteoraDammV2PoolProperties,
};

use crate::repositories::helper::{convert_i16_to_u8, convert_optional, convert_string_to_pubkey};

/// Row shape of `meteora_damm_v2_pool_properties` (baseline §8).
pub(super) struct MeteoraDammV2PoolPropertiesRow {
    pub(super) pool_address: String,
    pub(super) protocol_fee_percent: Option<i16>,
    pub(super) referral_fee_percent: Option<i16>,
    pub(super) base_fee_kind: Option<String>,
    pub(super) has_dynamic_fee: Option<bool>,
    pub(super) cliff_fee_numerator: Option<i64>,
    pub(super) number_of_period: Option<i32>,
    pub(super) period_frequency: Option<i64>,
    pub(super) reduction_factor: Option<i64>,
    pub(super) activation_point: Option<i64>,
    pub(super) activation_type: Option<i16>,
}

/// Rebuilds the decay curve, all or nothing: a partly NULL curve is corrupt, and would give
/// a confident wrong fee. ⚠️ Only the two time-scheduler kinds yield one — the decoder's gate,
/// restated because this side reads columns.
fn scheduler_from(row: &MeteoraDammV2PoolPropertiesRow) -> Option<FeeSchedulerParams> {
    let kind = match row.base_fee_kind.as_deref()? {
        "scheduler_linear" => BaseFeeKind::SchedulerLinear,
        "scheduler_exponential" => BaseFeeKind::SchedulerExponential,
        _ => return None,
    };
    Some(FeeSchedulerParams {
        cliff_fee_numerator: u64::try_from(row.cliff_fee_numerator?).ok()?,
        number_of_period: u16::try_from(row.number_of_period?).ok()?,
        period_frequency: u64::try_from(row.period_frequency?).ok()?,
        reduction_factor: u64::try_from(row.reduction_factor?).ok()?,
        activation_point: u64::try_from(row.activation_point?).ok()?,
        activation_type: u8::try_from(row.activation_type?).ok()?,
        kind,
    })
}

/// A percent outside `u8` surfaces as `Integrity`, not truncated.
impl TryFrom<MeteoraDammV2PoolPropertiesRow> for MeteoraDammV2PoolProperties {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2PoolPropertiesRow) -> Result<Self, Self::Error> {
        // Before the destructuring below moves `pool_address` out of `row`.
        let fee_scheduler = scheduler_from(&row);
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
            fee_scheduler,
            base_fee_kind: row.base_fee_kind,
            has_dynamic_fee: row.has_dynamic_fee,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
