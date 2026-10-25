use rust_decimal::Decimal;
use yog_core::domain::{MeteoraDammV2PoolProperties, PoolEntry, PoolValue};

use super::{MeteoraDammV2PropertiesResponse, PoolDetailResponse};
use crate::application::{PoolDetail, PoolProperties};
use crate::testing::{make_pool, pk};

fn detail(properties: Option<PoolProperties>) -> serde_json::Value {
    let detail = PoolDetail {
        entry: PoolEntry {
            pool: make_pool(pk(1), pk(2), pk(3)),
            token_a: None,
            token_b: None,
            state: None,
            value: PoolValue::default(),
        },
        properties,
        read_at: at(1_785_180_416),
    };
    serde_json::to_value(PoolDetailResponse::from(detail)).unwrap()
}

/// Mutation: the block serialized when absent, or nested away from the pool's own fields.
#[test]
fn the_protocol_block_sits_beside_the_pool_and_only_when_read() {
    let without = detail(None);
    let with = detail(Some(PoolProperties::MeteoraDammV2(scheduler_properties())));

    assert!(without.get("meteoraDammV2").is_none(), "{without}");
    assert_eq!(with["poolAddress"], pk(1).to_string());
    assert_eq!(with["meteoraDammV2"]["protocolFeePercent"], 20);
}

/// `28BDU1…`'s real curve: cliff 5000 bps decaying linearly to 400 over 144 periods of 600 s.
fn scheduler_properties() -> MeteoraDammV2PoolProperties {
    MeteoraDammV2PoolProperties {
        pool_address: pk(1),
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("scheduler_linear".to_string()),
        has_dynamic_fee: Some(true),
        fee_scheduler: Some(yog_core::amm::damm_v2::FeeSchedulerParams {
            cliff_fee_numerator: 500_000_000,
            number_of_period: 144,
            period_frequency: 600,
            reduction_factor: 3_194_444,
            activation_point: 1_785_180_416,
            activation_type: 1,
            kind: yog_core::amm::damm_v2::BaseFeeKind::SchedulerLinear,
        }),
    }
}

fn at(secs: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::TimeZone::timestamp_opt(&chrono::Utc, secs, 0).unwrap()
}

/// An expired scheduler: `feeBps` keeps the 5000 bps tier, `currentFeeBps` carries the 400 bps
/// floor a trader pays.
#[test]
fn an_expired_scheduler_reports_its_floor_beside_the_genesis_tier() {
    let expired = at(1_785_180_416 + 144 * 600 + 86_400);
    let r = MeteoraDammV2PropertiesResponse::build(scheduler_properties(), expired);

    // 500_000_000 − 144 × 3_194_444 = 40_000_064 → 400.00064 bps, to the digit.
    assert_eq!(
        r.current_fee_bps,
        Some(Decimal::new(40_000_064, 5)),
        "the floor is ~400 bps, not the 5000 bps cliff"
    );
    assert_eq!(r.fee_scheduler_expired, Some(true));
}

#[test]
fn a_live_scheduler_reports_a_fee_between_its_cliff_and_its_floor() {
    let mid = at(1_785_180_416 + 72 * 600);
    let r = MeteoraDammV2PropertiesResponse::build(scheduler_properties(), mid);

    let fee = r.current_fee_bps.expect("a live curve has a current fee");
    assert!(
        fee < Decimal::new(5000, 0) && fee > Decimal::new(400, 0),
        "expected a fee strictly inside (400, 5000), got {fee}"
    );
    assert_eq!(r.fee_scheduler_expired, Some(false));
}

/// A slot-activated curve is not evaluated: `None`, never a fall back to the cliff.
#[test]
fn a_slot_activated_scheduler_reports_no_current_fee_rather_than_its_cliff() {
    let mut props = scheduler_properties();
    props.fee_scheduler = props.fee_scheduler.map(|mut s| {
        s.activation_type = 0;
        s
    });
    let r = MeteoraDammV2PropertiesResponse::build(props, at(1_785_180_416 + 86_400));

    assert_eq!(r.current_fee_bps, None);
    assert_eq!(r.fee_scheduler_expired, None);
}

#[test]
fn a_constant_fee_reports_no_current_fee() {
    let mut props = scheduler_properties();
    props.base_fee_kind = Some("constant".to_string());
    props.fee_scheduler = None;
    let r = MeteoraDammV2PropertiesResponse::build(props, at(1_785_180_416));

    assert_eq!(r.current_fee_bps, None);
    assert_eq!(r.fee_scheduler_expired, None);
}

/// Mutation: `expired` derived apart from the evaluation — a linear curve decaying past its
/// cliff must silence both fields, as cp-amm's `safe_sub` errors.
#[test]
fn a_curve_that_cannot_be_evaluated_silences_both_fields() {
    let mut props = scheduler_properties();
    props.fee_scheduler = props.fee_scheduler.map(|mut s| {
        s.reduction_factor = s.cliff_fee_numerator;
        s
    });
    let r = MeteoraDammV2PropertiesResponse::build(props, at(1_785_180_416 + 86_400));

    assert_eq!(r.current_fee_bps, None);
    assert_eq!(
        r.fee_scheduler_expired, None,
        "expiry must not be reported for a curve whose fee could not be computed"
    );
}
