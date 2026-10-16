use rust_decimal::Decimal;
use yog_core::domain::{
    MeteoraDammV2PoolProperties, MeteoraDlmmPoolProperties, PoolAnalytics, PoolProperties,
};

use super::{MeteoraDammV2PropertiesResponse, MeteoraDlmmPropertiesResponse, PoolDetailResponse};
use crate::application::{EnrichedPool, EnrichedPoolDetail, EnrichedToken};
use crate::http::dto::PoolResponse;
use crate::http::dto::response::token::EmbeddedTokenResponse;
use crate::testing::{make_pool, pk};

// ⚠️ `#[serde(flatten)]` is easy to get subtly wrong: these lock the contract the web parses.

fn pool_response() -> PoolResponse {
    PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(None, None),
        EmbeddedTokenResponse::from_sources(None, None),
        PoolAnalytics::empty(),
        vec![],
    )
}

fn enriched_pool() -> EnrichedPool {
    EnrichedPool {
        pool: make_pool(pk(1), pk(2), pk(3)),
        token_a: EnrichedToken::unresolved(),
        token_b: EnrichedToken::unresolved(),
        analytics: PoolAnalytics::empty(),
        recent_signals: vec![],
    }
}

fn detail_response(properties: Option<MeteoraDammV2PropertiesResponse>) -> serde_json::Value {
    serde_json::to_value(PoolDetailResponse {
        pool: pool_response(),
        meteora_damm_v2: properties,
        meteora_dlmm: None,
    })
    .expect("detail response should serialize")
}

fn dlmm_detail_response(properties: Option<MeteoraDlmmPropertiesResponse>) -> serde_json::Value {
    serde_json::to_value(PoolDetailResponse {
        pool: pool_response(),
        meteora_damm_v2: None,
        meteora_dlmm: properties,
    })
    .expect("detail response should serialize")
}

/// The values read from `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` on mainnet.
fn dlmm_properties() -> MeteoraDlmmPropertiesResponse {
    MeteoraDlmmPropertiesResponse {
        bin_step: Some(1),
        base_factor: Some(10_000),
        base_fee_power_factor: Some(0),
        variable_fee_control: Some(2_000_000),
        max_volatility_accumulator: Some(100_000),
        protocol_share: Some(1_000),
    }
}

#[test]
fn pool_detail_keeps_shared_fields_at_top_level() {
    let json = detail_response(None);

    // Flattened: a client holding the list schema still parses a detail payload.
    assert!(json.get("poolAddress").is_some());
    assert!(json.get("protocol").is_some());
    assert!(json.get("tokenA").is_some());
    assert!(json.get("feeBps").is_some());
}

#[test]
fn pool_detail_omits_protocol_block_when_absent() {
    let json = detail_response(None);

    // Absent, not `null`.
    assert!(json.get("meteoraDammV2").is_none());
}

#[test]
fn pool_detail_nests_damm_v2_properties_under_their_protocol() {
    let json = detail_response(Some(MeteoraDammV2PropertiesResponse {
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
        current_fee_bps: None,
        fee_scheduler_expired: None,
    }));

    let block = json
        .get("meteoraDammV2")
        .expect("damm v2 block should be present");
    assert_eq!(block.get("protocolFeePercent").unwrap(), 20);
    assert_eq!(block.get("baseFeeKind").unwrap(), "constant");
    assert_eq!(block.get("hasDynamicFee").unwrap(), false);

    assert!(json.get("protocolFeePercent").is_none());
    assert!(json.get("baseFeeKind").is_none());
}

#[test]
fn pool_detail_nests_dlmm_properties_under_their_protocol() {
    let json = dlmm_detail_response(Some(dlmm_properties()));

    let block = json
        .get("meteoraDlmm")
        .expect("dlmm block should be present");
    assert_eq!(block.get("binStep").unwrap(), 1);
    assert_eq!(block.get("baseFactor").unwrap(), 10_000);
    assert_eq!(block.get("variableFeeControl").unwrap(), 2_000_000);
    assert_eq!(block.get("protocolShare").unwrap(), 1_000);

    // Nothing at the top level, and the sibling block absent rather than null.
    assert!(json.get("binStep").is_none());
    assert!(json.get("meteoraDammV2").is_none());
}

/// Neither protocol's key is ever present for the other protocol.
#[test]
fn the_two_protocol_blocks_are_mutually_exclusive() {
    let damm = detail_response(Some(MeteoraDammV2PropertiesResponse {
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
        current_fee_bps: None,
        fee_scheduler_expired: None,
    }));
    assert!(damm.get("meteoraDammV2").is_some());
    assert!(damm.get("meteoraDlmm").is_none());

    let dlmm = dlmm_detail_response(Some(dlmm_properties()));
    assert!(dlmm.get("meteoraDlmm").is_some());
    assert!(dlmm.get("meteoraDammV2").is_none());
}

/// An unresolved DLMM pool: the block present, every field `null` — unlike no block at all,
/// which means no satellite row.
#[test]
fn pool_detail_serializes_an_unresolved_dlmm_block_with_null_fields() {
    let json = dlmm_detail_response(Some(MeteoraDlmmPropertiesResponse {
        bin_step: None,
        base_factor: None,
        base_fee_power_factor: None,
        variable_fee_control: None,
        max_volatility_accumulator: None,
        protocol_share: None,
    }));

    let block = json.get("meteoraDlmm").expect("block should be present");
    assert!(block.get("binStep").unwrap().is_null());
    assert!(block.get("baseFactor").unwrap().is_null());
}

/// A domain `PoolProperties` lands in its own field and nowhere else, through the exhaustive
/// match of `From<EnrichedPoolDetail>`.
#[test]
fn the_from_impl_routes_each_protocol_to_its_own_block() {
    let dlmm = PoolDetailResponse::from(EnrichedPoolDetail {
        pool: enriched_pool(),
        properties: Some(PoolProperties::MeteoraDlmm(MeteoraDlmmPoolProperties {
            pool_address: pk(1),
            bin_step: Some(1),
            base_factor: Some(10_000),
            base_fee_power_factor: Some(0),
            variable_fee_control: Some(2_000_000),
            max_volatility_accumulator: Some(100_000),
            protocol_share: Some(1_000),
        })),
        evaluated_at: chrono::Utc::now(),
    });
    assert!(dlmm.meteora_dlmm.is_some(), "DLMM must reach its own block");
    assert!(dlmm.meteora_damm_v2.is_none());

    let damm = PoolDetailResponse::from(EnrichedPoolDetail {
        pool: enriched_pool(),
        properties: Some(PoolProperties::MeteoraDammV2(scheduler_properties())),
        evaluated_at: chrono::Utc::now(),
    });
    assert!(
        damm.meteora_damm_v2.is_some(),
        "DAMM v2 must reach its own block"
    );
    assert!(damm.meteora_dlmm.is_none());

    let none = PoolDetailResponse::from(EnrichedPoolDetail {
        pool: enriched_pool(),
        properties: None,
        evaluated_at: chrono::Utc::now(),
    });
    assert!(none.meteora_dlmm.is_none());
    assert!(none.meteora_damm_v2.is_none());
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
