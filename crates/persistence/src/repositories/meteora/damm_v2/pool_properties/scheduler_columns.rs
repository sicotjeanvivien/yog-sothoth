use yog_core::{amm::damm_v2::FeeSchedulerParams, domain::FieldOutOfRange};

/// The six fee-curve columns, as stored.
pub(super) struct SchedulerColumns {
    pub(super) cliff_fee_numerator: i64,
    pub(super) number_of_period: i32,
    pub(super) period_frequency: i64,
    pub(super) reduction_factor: i64,
    pub(super) activation_point: i64,
    pub(super) activation_type: i16,
}

/// The curve as `BIGINT`s, or the first `u64` that does not fit one (cp-amm bounds
/// `period_frequency` by `!= 0` only).
pub(super) fn scheduler_columns(
    s: &FeeSchedulerParams,
) -> Result<SchedulerColumns, FieldOutOfRange> {
    let bigint =
        |v: u64, field: &'static str| i64::try_from(v).map_err(|_| FieldOutOfRange { field });
    Ok(SchedulerColumns {
        cliff_fee_numerator: bigint(s.cliff_fee_numerator, "cliff_fee_numerator")?,
        number_of_period: i32::from(s.number_of_period),
        period_frequency: bigint(s.period_frequency, "period_frequency")?,
        reduction_factor: bigint(s.reduction_factor, "reduction_factor")?,
        activation_point: bigint(s.activation_point, "activation_point")?,
        activation_type: i16::from(s.activation_type),
    })
}

#[cfg(test)]
#[path = "scheduler_columns_tests.rs"]
mod tests;
