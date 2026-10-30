use chrono::Utc;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::*;

fn signal(detector: &str, value: Decimal) -> Signal {
    Signal {
        detector: detector.to_string(),
        protocol: Protocol::MeteoraDammV2,
        pool_address: Pubkey::new_unique(),
        severity: Severity::Warning,
        value,
        threshold: None,
        message: None,
        triggered_at: Utc::now(),
    }
}

// Mutation: a tag retyped apart from the detector's own, or `tvl_drain` given a sign.
#[test]
fn each_detector_has_the_unit_of_its_values() {
    assert_eq!(
        SignalValueUnit::of("price_oracle_deviation"),
        SignalValueUnit::SignedRatio
    );
    assert_eq!(
        SignalValueUnit::of("flow_imbalance"),
        SignalValueUnit::SignedRatio
    );
    assert_eq!(SignalValueUnit::of("tvl_drain"), SignalValueUnit::Ratio);
}

// Mutation: an unknown detector read as a ratio renders its own unit as a percent.
#[test]
fn an_unknown_detector_stays_in_its_own_unit() {
    assert_eq!(SignalValueUnit::of("later_detector"), SignalValueUnit::Raw);
}

// Mutation: the two tokens swapped sends every lean to the token it left.
#[test]
fn a_flow_leans_toward_the_token_its_sign_names() {
    let toward = |value| signal(DETECTOR_FLOW_IMBALANCE, value).flows_toward();
    assert_eq!(toward(Decimal::new(75, 2)), Some(PairToken::TokenB));
    assert_eq!(toward(Decimal::new(-75, 2)), Some(PairToken::TokenA));
}

// Mutation: the zero guard dropped gives a balanced flow a direction.
#[test]
fn a_balanced_flow_leans_nowhere() {
    assert_eq!(
        signal(DETECTOR_FLOW_IMBALANCE, Decimal::ZERO).flows_toward(),
        None
    );
}

// Mutation: the detector guard dropped reads a price deviation's sign as a flow's.
#[test]
fn only_a_flow_imbalance_has_a_direction() {
    let value = Decimal::new(-75, 2);
    assert_eq!(
        signal(DETECTOR_PRICE_ORACLE_DEVIATION, value).flows_toward(),
        None
    );
    assert_eq!(signal(DETECTOR_TVL_DRAIN, value).flows_toward(), None);
}
