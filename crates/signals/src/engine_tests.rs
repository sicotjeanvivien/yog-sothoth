//! Unit tests for the cooldown / escalation dedup filter. DB-free: the
//! `recent` map stands in for what the repository would return.

use super::*;
use chrono::Utc;
use rust_decimal::Decimal;
use yog_core::domain::Protocol;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn sig(pool: u8, severity: Severity) -> Signal {
    Signal {
        detector: "flow_imbalance".to_string(),
        protocol: Protocol::MeteoraDammV2,
        pool_address: pk(pool),
        severity,
        value: Decimal::ONE,
        threshold: None,
        message: None,
        triggered_at: Utc::now(),
    }
}

fn pools(emitted: &[Signal]) -> Vec<u8> {
    emitted
        .iter()
        .map(|s| s.pool_address.to_bytes()[0])
        .collect()
}

#[test]
fn new_pool_is_emitted() {
    let recent = HashMap::new();
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert_eq!(pools(&out), vec![1]);
}

#[test]
fn same_severity_within_cooldown_is_suppressed() {
    let recent = HashMap::from([(pk(1), Severity::Warning)]);
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert!(out.is_empty());
}

#[test]
fn escalation_breaks_the_cooldown() {
    // Already emitted Warning; a Critical for the same pool must get through.
    let recent = HashMap::from([(pk(1), Severity::Warning)]);
    let out = emittable(vec![sig(1, Severity::Critical)], &recent);
    assert_eq!(pools(&out), vec![1]);
    assert_eq!(out[0].severity, Severity::Critical);
}

#[test]
fn de_escalation_is_suppressed() {
    // Already Critical; a lower Warning must not re-alert.
    let recent = HashMap::from([(pk(1), Severity::Critical)]);
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert!(out.is_empty());
}

#[test]
fn mixed_batch_keeps_only_new_and_escalating() {
    let recent = HashMap::from([
        (pk(1), Severity::Warning),  // pool 1: seen Warning
        (pk(2), Severity::Critical), // pool 2: seen Critical
    ]);
    let out = emittable(
        vec![
            sig(1, Severity::Warning),  // same → drop
            sig(2, Severity::Critical), // same → drop
            sig(3, Severity::Warning),  // new pool → keep
            sig(1, Severity::Critical), // escalation → keep
        ],
        &recent,
    );
    assert_eq!(pools(&out), vec![3, 1]);
}
