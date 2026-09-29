use super::*;

/// Each refusal names its own variable — two variables share this file, and a
/// refusal quoting the wrong one sends the operator to the wrong line.
fn key_of(result: Result<impl std::fmt::Debug, ConfigError>) -> String {
    match result {
        Err(ConfigError::InvalidValue { key, .. }) => key,
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn the_interval_is_refused_at_zero_and_kept_otherwise() {
    assert_eq!(interval_secs(600).unwrap(), Duration::from_secs(600));
    assert_eq!(
        key_of(interval_secs(0)),
        "SIGNALS_MATERIALIZATION_INTERVAL_SECS"
    );
}

#[test]
fn the_limit_refuses_zero_and_what_a_duration_cannot_hold() {
    assert_eq!(max_wait_minutes(240).unwrap(), ChronoDuration::hours(4));
    for minutes in [0, u64::MAX, i64::MAX as u64] {
        assert_eq!(
            key_of(max_wait_minutes(minutes)),
            "SIGNALS_MATERIALIZATION_MAX_WAIT_MINS",
            "{minutes}"
        );
    }
}
