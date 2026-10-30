use yog_core::domain;

use solana_pubkey::Pubkey;

use super::{PairTokenResponse, SeverityResponse, SignalResponse, SignalValueUnitResponse};
use crate::application::EnrichedSignal;
use crate::testing::{make_signal_record, wire_tag};

#[test]
fn severity_matches_the_domain() {
    use domain::Severity::*;
    for v in [Info, Warning, Critical] {
        assert_eq!(wire_tag(SeverityResponse::from(v)), v.as_str());
    }
}

// Mutation: a variant mapped to its neighbour shows a drain with a sign, or the wrong token.
#[test]
fn the_meaning_of_a_value_keeps_its_wire_tags() {
    use domain::{PairToken, SignalValueUnit};
    let unit = |v: SignalValueUnit| wire_tag(SignalValueUnitResponse::from(v));
    assert_eq!(unit(SignalValueUnit::SignedRatio), "signedRatio");
    assert_eq!(unit(SignalValueUnit::Ratio), "ratio");
    assert_eq!(unit(SignalValueUnit::Raw), "raw");
    let token = |v: PairToken| wire_tag(PairTokenResponse::from(v));
    assert_eq!(token(PairToken::TokenA), "tokenA");
    assert_eq!(token(PairToken::TokenB), "tokenB");
}

// Mutation: the unit read off a fixed detector, or the direction left out of the response.
#[test]
fn a_signal_says_what_its_value_means() {
    let response = |detector: &str| {
        let mut record = make_signal_record(1, Pubkey::new_unique());
        record.signal.detector = detector.to_string();
        serde_json::to_value(SignalResponse::from(EnrichedSignal::bare(record))).unwrap()
    };

    let flow = response(domain::DETECTOR_FLOW_IMBALANCE);
    assert_eq!(flow["valueUnit"], "signedRatio");
    assert_eq!(flow["flowsToward"], "tokenB");

    let drain = response(domain::DETECTOR_TVL_DRAIN);
    assert_eq!(drain["valueUnit"], "ratio");
    assert_eq!(drain["flowsToward"], serde_json::Value::Null);
}
