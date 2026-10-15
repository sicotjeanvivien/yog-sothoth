use super::{PoolResponse, PoolSignalResponse, ProtocolResponse, effective_fee_bps};
use crate::http::dto::EmbeddedTokenResponse;
use crate::testing::{make_pool, make_signal_record, pk, wire_tag};
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
fn protocol_matches_the_domain() {
    for p in yog_core::domain::Protocol::all() {
        assert_eq!(wire_tag(ProtocolResponse::from(*p)), p.as_str());
    }
}

#[test]
fn pool_signal_response_carries_severity_detector_and_time() {
    let record = make_signal_record(7, pk(1));
    let expected_at = record.signal.triggered_at;

    let resp = PoolSignalResponse::from(record);

    // Severity crosses the wire as its snake_case tag, like the feed's
    // SignalResponse.
    assert_eq!(serde_json::to_value(&resp).unwrap()["severity"], "warning");
    assert_eq!(resp.detector, "flow_imbalance");
    assert_eq!(resp.triggered_at, expected_at);
}

// ── The coverage counters must reach the wire ────────────────────────────────

/// Found in self-review: nothing asserted that the two coverage counters
/// survive serialisation. Every service test builds analytics with
/// `..PoolAnalytics::empty()`, so the fields were always 0 and never inspected
/// in a JSON body — a rename or a missed wiring in `PoolResponse::new` would
/// have shipped silently, and the whole point of this pair is to be *visible*.
#[test]
fn pool_response_serialises_the_coverage_counters_in_camel_case() {
    let analytics = PoolAnalytics {
        tvl_usd: None,
        volume_24h_usd: Some(Decimal::new(1_400_000, 0)),
        fees_24h_usd: Some(Decimal::new(3_500, 0)),
        protocol_fees_24h_usd: Some(Decimal::new(700, 0)),
        referral_fees_24h_usd: Some(Decimal::new(140, 0)),
        lp_fees_24h_usd: Some(Decimal::new(2_660, 0)),
        swap_buckets_24h: 24,
        swap_buckets_priced_24h: 14,
    };

    let resp = PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(Some(pk(2)), None, None),
        EmbeddedTokenResponse::from_sources(Some(pk(3)), None, None),
        analytics,
        Vec::new(),
    );
    let json = serde_json::to_value(&resp).expect("PoolResponse must serialise");

    assert_eq!(json["swapBuckets24h"], 24);
    assert_eq!(json["swapBucketsPriced24h"], 14);
    // The value it qualifies is a sub-total, and stays present: the pair is what
    // says so, not the absence of a number. Pinned as a STRING — rust_decimal
    // serialises decimals that way by default, which is what the web's
    // `BigDecimal` zod type expects and what preserves the trailing digits the
    // SQL produces. (The struct doc claimed "JSON numbers"; it was wrong.)
    assert_eq!(json["volume24hUsd"], serde_json::json!("1400000"));
}

// ── The fee split reaches the wire, and is not recomputed ────────────────────

/// `lpFees24hUsd` was derived here as `fees - protocol`, which credits the
/// referral to the LPs (corrected by migration 007). It is now read from the
/// analytics, where the split is computed once in SQL.
///
/// The fixture makes the two answers differ — `3500 - 700 = 2800` against a
/// true LP share of `2660` — so this fails if the subtraction ever comes back.
#[test]
fn pool_response_carries_the_three_fee_shares_without_recomputing_them() {
    let analytics = PoolAnalytics {
        tvl_usd: None,
        volume_24h_usd: Some(Decimal::new(1_400_000, 0)),
        fees_24h_usd: Some(Decimal::new(3_500, 0)),
        protocol_fees_24h_usd: Some(Decimal::new(700, 0)),
        referral_fees_24h_usd: Some(Decimal::new(140, 0)),
        lp_fees_24h_usd: Some(Decimal::new(2_660, 0)),
        swap_buckets_24h: 24,
        swap_buckets_priced_24h: 24,
    };

    let resp = PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(Some(pk(2)), None, None),
        EmbeddedTokenResponse::from_sources(Some(pk(3)), None, None),
        analytics,
        Vec::new(),
    );
    let json = serde_json::to_value(&resp).expect("PoolResponse must serialise");

    assert_eq!(json["fees24hUsd"], serde_json::json!("3500"));
    assert_eq!(json["protocolFees24hUsd"], serde_json::json!("700"));
    assert_eq!(json["referralFees24hUsd"], serde_json::json!("140"));
    assert_eq!(
        json["lpFees24hUsd"],
        serde_json::json!("2660"),
        "the LP share is claiming + compounding, not fees - protocol"
    );
}

/// A window covered end to end must be distinguishable from a quiet pool: the
/// first reports `n / n`, the second `0 / 0`. Collapsing both to "no hint" is
/// how a missing figure gets read as a complete one.
#[test]
fn a_quiet_pool_reports_zero_buckets_not_full_coverage() {
    let resp = PoolResponse::new(
        make_pool(pk(1), pk(2), pk(3)),
        EmbeddedTokenResponse::from_sources(Some(pk(2)), None, None),
        EmbeddedTokenResponse::from_sources(Some(pk(3)), None, None),
        PoolAnalytics::empty(),
        Vec::new(),
    );
    let json = serde_json::to_value(&resp).expect("PoolResponse must serialise");

    assert_eq!(json["swapBuckets24h"], 0);
    assert_eq!(json["swapBucketsPriced24h"], 0);
    assert_eq!(json["volume24hUsd"], serde_json::Value::Null);
}
