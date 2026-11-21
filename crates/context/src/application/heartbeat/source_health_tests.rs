//! The verdict on a window, from what the workers recorded.

use super::*;

const REFUSED: &str = "source rate-limited (429), retry_after=None";

#[test]
fn a_source_refusing_every_call_fails_the_window_by_its_role() {
    // Mutation: refusals ignored by the verdict.
    let health = SourceHealth::default();
    for _ in 0..3 {
        health.record_failure(SourceRole::Metadata, REFUSED.to_string());
    }

    assert_eq!(
        health.take_window(),
        WindowVerdict::Failing(format!(
            "metadata: 3 calls, none succeeded, last error: {REFUSED}"
        ))
    );
}

#[test]
fn one_refusal_followed_by_a_success_leaves_the_window_healthy() {
    // Mutation: every refusal fails the window.
    let health = SourceHealth::default();
    health.record_failure(SourceRole::PoolAccount, REFUSED.to_string());
    health.record_success(SourceRole::PoolAccount);

    assert_eq!(health.take_window(), WindowVerdict::Healthy);
}

#[test]
fn a_window_without_a_call_is_healthy() {
    // Mutation: an empty window fails.
    assert_eq!(
        SourceHealth::default().take_window(),
        WindowVerdict::Healthy
    );
}

#[test]
fn a_failed_window_does_not_carry_over_into_the_next() {
    // Mutation: `take_window` reads without clearing.
    let health = SourceHealth::default();
    health.record_failure(SourceRole::Price, REFUSED.to_string());
    assert!(matches!(health.take_window(), WindowVerdict::Failing(_)));

    assert_eq!(health.take_window(), WindowVerdict::Healthy);
}

#[test]
fn a_healthy_source_does_not_hide_a_failing_one() {
    // Mutation: one success, of any source, clears the window.
    let health = SourceHealth::default();
    health.record_success(SourceRole::Price);
    health.record_failure(SourceRole::Metadata, REFUSED.to_string());

    let WindowVerdict::Failing(reason) = health.take_window() else {
        panic!("the metadata source refused every call");
    };
    assert!(reason.starts_with("metadata:"), "{reason}");
    assert!(!reason.contains("price"), "{reason}");
}

#[test]
fn every_failing_source_is_named() {
    // Mutation: only the first failing source is named.
    let health = SourceHealth::default();
    health.record_failure(SourceRole::Metadata, REFUSED.to_string());
    health.record_failure(SourceRole::Price, "no price returned".to_string());

    assert_eq!(
        health.take_window(),
        WindowVerdict::Failing(format!(
            "metadata: 1 calls, none succeeded, last error: {REFUSED}; \
             price: 1 calls, none succeeded, last error: no price returned"
        ))
    );
}
