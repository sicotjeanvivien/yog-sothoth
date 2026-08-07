use yog_core::{
    RepositoryError,
    amm::damm_v2::{BaseFeeKind, FeeSchedulerParams},
    domain::MeteoraDammV2PoolProperties,
};

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
    pub(super) cliff_fee_numerator: Option<i64>,
    pub(super) number_of_period: Option<i32>,
    pub(super) period_frequency: Option<i64>,
    pub(super) reduction_factor: Option<i64>,
    pub(super) activation_point: Option<i64>,
    pub(super) activation_type: Option<i16>,
}

/// Rebuild the decay curve from its six columns — **all or nothing**.
///
/// They are written as a unit by one account read, so a row with some of them
/// set and others NULL is not a partially-usable curve, it is a corrupt one:
/// evaluating a decay without its period length or its origin would produce a
/// confident wrong fee. `?` on each therefore collapses any such row to `None`,
/// which every consumer already handles as "no current fee available".
///
/// The `kind` comes from `base_fee_kind`, and only the two time-scheduler values
/// yield a curve — the same gate the decoder applies, restated here because this
/// side reads columns rather than bytes and cannot inherit it.
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

/// The percent columns only ever hold values written from a `u8` (0..=100), but
/// a corrupt row surfaces as `Integrity` rather than being silently truncated —
/// the guard is `convert_i16_to_u8`, lifted over `Option` by `convert_optional`.
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
