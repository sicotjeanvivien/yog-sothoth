use super::{PoolSignalResponse, effective_fee_bps};
use crate::testing::{make_signal_record, pk};
use rust_decimal::Decimal;

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
