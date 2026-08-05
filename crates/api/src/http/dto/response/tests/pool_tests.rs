use super::{
    MeteoraDammV2PropertiesResponse, MeteoraDlmmPropertiesResponse, PoolDetailResponse,
    PoolResponse, PoolSignalResponse, effective_fee_bps,
};
use crate::application::{EnrichedPool, EnrichedPoolDetail, EnrichedToken};
use crate::http::dto::EmbeddedTokenResponse;
use crate::testing::{make_pool, make_signal_record, pk};
use rust_decimal::Decimal;
use yog_core::domain::{MeteoraDlmmPoolProperties, PoolAnalytics, PoolProperties};

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
// PoolDetailResponse wire shape (baseline §8)
// ---------------------------------------------------------------------------
//
// The detail payload is the only place the cp-amm fee properties still cross
// the wire, and `#[serde(flatten)]` is easy to get subtly wrong. These lock the
// contract the web client parses.

fn pool_response() -> PoolResponse {
    PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(None, None, None),
        EmbeddedTokenResponse::from_sources(None, None, None),
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

/// The values read from `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` on
/// mainnet — the same anchor the decoder's fixtures use.
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

    // Nothing leaks to the top level, and the sibling protocol's key is absent
    // rather than null — the two blocks are mutually exclusive on the wire.
    assert!(json.get("binStep").is_none());
    assert!(json.get("meteoraDammV2").is_none());
}

/// The two protocol blocks are siblings, so a DAMM v2 payload must be parseable
/// by a client that only knows about DLMM and vice versa: neither key is ever
/// present for the wrong protocol.
#[test]
fn the_two_protocol_blocks_are_mutually_exclusive() {
    let damm = detail_response(Some(MeteoraDammV2PropertiesResponse {
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
    }));
    assert!(damm.get("meteoraDammV2").is_some());
    assert!(damm.get("meteoraDlmm").is_none());

    let dlmm = dlmm_detail_response(Some(dlmm_properties()));
    assert!(dlmm.get("meteoraDlmm").is_some());
    assert!(dlmm.get("meteoraDammV2").is_none());
}

/// An unresolved DLMM pool: the block is present but every field is `null`.
///
/// Distinct from "no block at all", which means no satellite row. The
/// difference matters to a client deciding between "not indexed yet" and "this
/// protocol has no such properties".
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

/// The routing itself: a domain `PoolProperties` must land in its own field and
/// nowhere else. This goes through the `From<EnrichedPoolDetail>` impl — the
/// exhaustive match that adding a protocol forces someone to extend.
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
    });
    assert!(dlmm.meteora_dlmm.is_some(), "DLMM must reach its own block");
    assert!(dlmm.meteora_damm_v2.is_none());

    let none = PoolDetailResponse::from(EnrichedPoolDetail {
        pool: enriched_pool(),
        properties: None,
    });
    assert!(none.meteora_dlmm.is_none());
    assert!(none.meteora_damm_v2.is_none());
}
