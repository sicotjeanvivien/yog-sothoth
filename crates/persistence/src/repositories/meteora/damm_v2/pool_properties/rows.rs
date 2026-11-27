use yog_core::{
    RepositoryError,
    amm::damm_v2::{BaseFeeKind, FeeSchedulerParams},
    domain::{FeeSchedulerDrop, FieldOutOfRange, MeteoraDammV2PoolProperties},
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
fn scheduler_from(
    row: &MeteoraDammV2PoolPropertiesRow,
) -> Result<Option<FeeSchedulerParams>, FeeSchedulerDrop> {
    let Some(kind) = row.base_fee_kind.as_deref().and_then(|stored| {
        [
            BaseFeeKind::SchedulerLinear,
            BaseFeeKind::SchedulerExponential,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == stored)
    }) else {
        return Ok(None);
    };
    let (
        Some(cliff_fee_numerator),
        Some(number_of_period),
        Some(period_frequency),
        Some(reduction_factor),
        Some(activation_point),
        Some(activation_type),
    ) = (
        row.cliff_fee_numerator,
        row.number_of_period,
        row.period_frequency,
        row.reduction_factor,
        row.activation_point,
        row.activation_type,
    )
    else {
        // Six NULLs: no curve was stored (dropped and counted at the write, or a short account).
        let any_stored = row.cliff_fee_numerator.is_some()
            || row.number_of_period.is_some()
            || row.period_frequency.is_some()
            || row.reduction_factor.is_some()
            || row.activation_point.is_some()
            || row.activation_type.is_some();
        return if any_stored {
            Err(FeeSchedulerDrop::PartlyNull)
        } else {
            Ok(None)
        };
    };
    Ok(Some(FeeSchedulerParams {
        cliff_fee_numerator: narrow(cliff_fee_numerator, "cliff_fee_numerator")?,
        number_of_period: narrow(number_of_period, "number_of_period")?,
        period_frequency: narrow(period_frequency, "period_frequency")?,
        reduction_factor: narrow(reduction_factor, "reduction_factor")?,
        activation_point: narrow(activation_point, "activation_point")?,
        activation_type: narrow(activation_type, "activation_type")?,
        kind,
    }))
}

fn narrow<S, T: TryFrom<S>>(value: S, field: &'static str) -> Result<T, FeeSchedulerDrop> {
    T::try_from(value).map_err(|_| FeeSchedulerDrop::OutOfRange(FieldOutOfRange { field }))
}

/// A percent outside `u8` surfaces as `Integrity`, not truncated.
impl TryFrom<MeteoraDammV2PoolPropertiesRow> for MeteoraDammV2PoolProperties {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2PoolPropertiesRow) -> Result<Self, Self::Error> {
        // Before the destructuring below moves `pool_address` out of `row`.
        let (fee_scheduler, fee_scheduler_dropped) = match scheduler_from(&row) {
            Ok(curve) => (curve, None),
            Err(drop) => (None, Some(drop)),
        };
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
            fee_scheduler_dropped,
            base_fee_kind: row.base_fee_kind,
            has_dynamic_fee: row.has_dynamic_fee,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
