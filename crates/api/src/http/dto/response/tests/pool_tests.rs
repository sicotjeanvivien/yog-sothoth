use super::{
    MeteoraDammV2PropertiesResponse, PoolDetailResponse, PoolResponse, PoolSignalResponse,
    effective_fee_bps,
};
use crate::http::dto::EmbeddedTokenResponse;
use crate::testing::{make_pool, make_signal_record, pk};
use rust_decimal::Decimal;
use yog_core::domain::PoolAnalytics;

#[test]
fn effective_fee_bps_is_fees_over_volume_in_bps() {
    // 25 USD fees on 10_000 USD volume = 0.25% = 25 bps.
    let bps = effective_fee_bps(Some(Decimal::new(25, 0)), Some(Decimal::new(10_000, 0)));
    assert_eq!(bps, Some(Decimal::new(25, 0)));
}

#[test]
fn effective_fee_bps_none_when_volume_zero() {
    assert_eq!(
        effective_fee_bps(Some(Decimal::new(25, 0)), Some(Decimal::ZERO)),
        None
    );
}

#[test]
fn effective_fee_bps_none_when_an_input_missing() {
    assert_eq!(effective_fee_bps(None, Some(Decimal::new(10, 0))), None);
    assert_eq!(effective_fee_bps(Some(Decimal::new(10, 0)), None), None);
}

#[test]
fn pool_signal_response_carries_severity_detector_and_time() {
    let record = make_signal_record(7, pk(1));
    let expected_at = record.signal.triggered_at;

    let resp = PoolSignalResponse::from(record);

    // Severity crosses the wire as its snake_case tag, like the feed's
    // SignalResponse.
    assert_eq!(resp.severity, "warning");
    assert_eq!(resp.detector, "flow_imbalance");
    assert_eq!(resp.triggered_at, expected_at);
}

// ---------------------------------------------------------------------------
// PoolDetailResponse wire shape (migration 036)
// ---------------------------------------------------------------------------
//
// The detail payload is the only place the cp-amm fee properties still cross
// the wire, and `#[serde(flatten)]` is easy to get subtly wrong. These lock the
// contract the web client parses.

fn detail_response(properties: Option<MeteoraDammV2PropertiesResponse>) -> serde_json::Value {
    let pool = PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(None, None, None),
        EmbeddedTokenResponse::from_sources(None, None, None),
        PoolAnalytics::empty(),
        vec![],
    );
    serde_json::to_value(PoolDetailResponse {
        pool,
        meteora_damm_v2: properties,
    })
    .expect("detail response should serialize")
}

#[test]
fn pool_detail_keeps_shared_fields_at_top_level() {
    let json = detail_response(None);

    // Flattened: a client holding the *list* schema can still parse a detail
    // payload. If this regresses, every consumer of PoolSchema breaks at once.
    assert!(json.get("poolAddress").is_some());
    assert!(json.get("protocol").is_some());
    assert!(json.get("tokenA").is_some());
    assert!(json.get("feeBps").is_some());
}

#[test]
fn pool_detail_omits_protocol_block_when_absent() {
    let json = detail_response(None);

    // `skip_serializing_if` — absent, not `null`. A DLMM pool (or a DAMM v2 pool
    // with no satellite row yet) simply has no block.
    assert!(json.get("meteoraDammV2").is_none());
}

#[test]
fn pool_detail_nests_damm_v2_properties_under_their_protocol() {
    let json = detail_response(Some(MeteoraDammV2PropertiesResponse {
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
    }));

    let block = json
        .get("meteoraDammV2")
        .expect("damm v2 block should be present");
    assert_eq!(block.get("protocolFeePercent").unwrap(), 20);
    assert_eq!(block.get("baseFeeKind").unwrap(), "constant");
    assert_eq!(block.get("hasDynamicFee").unwrap(), false);

    // …and NOT at the top level: that is the whole point of the split.
    assert!(json.get("protocolFeePercent").is_none());
    assert!(json.get("baseFeeKind").is_none());
}
