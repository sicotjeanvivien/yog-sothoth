use std::env;
use std::sync::Mutex;

use super::*;

/// Held by `load_with` while it writes and reads the environment; poison is stepped over, the panicking test
/// having already failed.
static ENV: Mutex<()> = Mutex::new(());

const OPTIONAL: [&str; 22] = [
    "SIGNALS_FLOW_INTERVAL_SECS",
    "SIGNALS_FLOW_WINDOW_HOURS",
    "SIGNALS_FLOW_COOLDOWN_HOURS",
    "SIGNALS_FLOW_MIN_VOLUME_USD",
    "SIGNALS_FLOW_THRESHOLD",
    "SIGNALS_FLOW_CRITICAL",
    "SIGNALS_PRICE_DEVIATION_INTERVAL_SECS",
    "SIGNALS_PRICE_DEVIATION_COOLDOWN_HOURS",
    "SIGNALS_PRICE_DEVIATION_MAX_PRICE_AGE_MINS",
    "SIGNALS_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS",
    "SIGNALS_PRICE_DEVIATION_MIN_TVL_USD",
    "SIGNALS_PRICE_DEVIATION_THRESHOLD",
    "SIGNALS_PRICE_DEVIATION_CRITICAL",
    "SIGNALS_TVL_DRAIN_INTERVAL_SECS",
    "SIGNALS_TVL_DRAIN_WINDOW_HOURS",
    "SIGNALS_TVL_DRAIN_COOLDOWN_HOURS",
    "SIGNALS_TVL_DRAIN_MIN_TVL_USD",
    "SIGNALS_TVL_DRAIN_THRESHOLD",
    "SIGNALS_TVL_DRAIN_CRITICAL",
    "SIGNALS_MATERIALIZATION_INTERVAL_SECS",
    "SIGNALS_MATERIALIZATION_MAX_WAIT_MINS",
    "SIGNALS_MATERIALIZATION_HEARTBEAT_URL",
];

/// `Config::load` with the required URL, `set`, and no other optional variable.
fn load_with(set: &[(&str, &str)]) -> Result<Config, ConfigError> {
    let _env = ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: `set_var` is unsound while another thread reads the environment; `ENV` serialises the
    // writers, and no other test of this crate reads it directly.
    unsafe {
        for key in OPTIONAL {
            env::remove_var(key);
        }
        env::set_var("DATABASE_URL_SIGNALS", "postgresql://u:p@localhost:5433/db");
        for (key, value) in set {
            env::set_var(key, value);
        }
    }
    Config::load()
}

fn hours(n: u64) -> Duration {
    Duration::from_secs(n * 3600)
}

/// Mutation: a detector's default changed, or read in the wrong unit.
#[test]
fn without_a_variable_each_setting_has_its_default() {
    let config = load_with(&[]).expect("the defaults load");

    let flow = config.flow_imbalance_settings();
    assert_eq!(flow.interval, Duration::from_secs(300));
    assert_eq!(flow.window, ChronoDuration::hours(24));
    assert_eq!(flow.cooldown, hours(6));
    assert_eq!(flow.min_volume_usd, Decimal::from(10_000));
    assert_eq!(flow.threshold, Decimal::new(6, 1));
    assert_eq!(flow.critical, Decimal::new(9, 1));

    let price = config.price_oracle_deviation_settings();
    assert_eq!(price.interval, Duration::from_secs(300));
    assert_eq!(price.cooldown, hours(6));
    assert_eq!(price.max_price_age, ChronoDuration::minutes(15));
    assert_eq!(price.max_spot_age, ChronoDuration::hours(24));
    assert_eq!(price.min_tvl_usd, Decimal::from(10_000));
    assert_eq!(price.threshold, Decimal::new(5, 2));
    assert_eq!(price.critical, Decimal::new(2, 1));

    let tvl = config.tvl_drain_settings();
    assert_eq!(tvl.interval, Duration::from_secs(300));
    assert_eq!(tvl.window, ChronoDuration::hours(6));
    assert_eq!(tvl.cooldown, hours(6));
    assert_eq!(tvl.min_tvl_usd, Decimal::from(10_000));
    assert_eq!(tvl.threshold, Decimal::new(5, 1));
    assert_eq!(tvl.critical, Decimal::new(8, 1));
}

/// Mutation: two variable names swapped in `load`, two fields swapped in a settings function, or
/// hours read as seconds.
#[test]
fn each_variable_reaches_the_setting_of_its_detector() {
    let config = load_with(&[
        ("SIGNALS_FLOW_INTERVAL_SECS", "101"),
        ("SIGNALS_FLOW_WINDOW_HOURS", "102"),
        ("SIGNALS_FLOW_COOLDOWN_HOURS", "103"),
        ("SIGNALS_FLOW_MIN_VOLUME_USD", "104"),
        ("SIGNALS_FLOW_THRESHOLD", "0.11"),
        ("SIGNALS_FLOW_CRITICAL", "0.12"),
        ("SIGNALS_PRICE_DEVIATION_INTERVAL_SECS", "201"),
        ("SIGNALS_PRICE_DEVIATION_COOLDOWN_HOURS", "202"),
        ("SIGNALS_PRICE_DEVIATION_MAX_PRICE_AGE_MINS", "203"),
        ("SIGNALS_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS", "204"),
        ("SIGNALS_PRICE_DEVIATION_MIN_TVL_USD", "205"),
        ("SIGNALS_PRICE_DEVIATION_THRESHOLD", "0.21"),
        ("SIGNALS_PRICE_DEVIATION_CRITICAL", "0.22"),
        ("SIGNALS_TVL_DRAIN_INTERVAL_SECS", "301"),
        ("SIGNALS_TVL_DRAIN_WINDOW_HOURS", "302"),
        ("SIGNALS_TVL_DRAIN_COOLDOWN_HOURS", "303"),
        ("SIGNALS_TVL_DRAIN_MIN_TVL_USD", "304"),
        ("SIGNALS_TVL_DRAIN_THRESHOLD", "0.31"),
        ("SIGNALS_TVL_DRAIN_CRITICAL", "0.32"),
    ])
    .expect("distinct values load");

    let flow = config.flow_imbalance_settings();
    assert_eq!(flow.interval, Duration::from_secs(101));
    assert_eq!(flow.window, ChronoDuration::hours(102));
    assert_eq!(flow.cooldown, hours(103));
    assert_eq!(flow.min_volume_usd, Decimal::from(104));
    assert_eq!(flow.threshold, Decimal::new(11, 2));
    assert_eq!(flow.critical, Decimal::new(12, 2));

    let price = config.price_oracle_deviation_settings();
    assert_eq!(price.interval, Duration::from_secs(201));
    assert_eq!(price.cooldown, hours(202));
    assert_eq!(price.max_price_age, ChronoDuration::minutes(203));
    assert_eq!(price.max_spot_age, ChronoDuration::hours(204));
    assert_eq!(price.min_tvl_usd, Decimal::from(205));
    assert_eq!(price.threshold, Decimal::new(21, 2));
    assert_eq!(price.critical, Decimal::new(22, 2));

    let tvl = config.tvl_drain_settings();
    assert_eq!(tvl.interval, Duration::from_secs(301));
    assert_eq!(tvl.window, ChronoDuration::hours(302));
    assert_eq!(tvl.cooldown, hours(303));
    assert_eq!(tvl.min_tvl_usd, Decimal::from(304));
    assert_eq!(tvl.threshold, Decimal::new(31, 2));
    assert_eq!(tvl.critical, Decimal::new(32, 2));
}

/// Mutation: the trim removed, or an unreadable value falling back to the default without a word.
#[test]
fn a_decimal_is_read_past_a_crlf_and_refused_when_unreadable() {
    let config = load_with(&[("SIGNALS_FLOW_THRESHOLD", "0.33\r")]).expect("a CRLF `.env` loads");
    assert_eq!(
        config.flow_imbalance_settings().threshold,
        Decimal::new(33, 2)
    );

    match load_with(&[("SIGNALS_FLOW_THRESHOLD", "abc")]) {
        Err(ConfigError::InvalidValue { key, expected, .. }) => {
            assert_eq!(key, "SIGNALS_FLOW_THRESHOLD");
            assert_eq!(expected, "a decimal number");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Mutation: one of the three `validate_ladder` removed, quoting another detector's variable, or
/// `decimal_var` refusing the value first.
#[test]
fn a_threshold_reaching_its_critical_is_refused_under_its_own_variable() {
    for (threshold_key, critical_key) in [
        ("SIGNALS_FLOW_THRESHOLD", "SIGNALS_FLOW_CRITICAL"),
        (
            "SIGNALS_PRICE_DEVIATION_THRESHOLD",
            "SIGNALS_PRICE_DEVIATION_CRITICAL",
        ),
        ("SIGNALS_TVL_DRAIN_THRESHOLD", "SIGNALS_TVL_DRAIN_CRITICAL"),
    ] {
        for threshold in ["0.7", "0.8"] {
            match load_with(&[(threshold_key, threshold), (critical_key, "0.7")]) {
                Err(ConfigError::InvalidValue { key, expected, .. }) => {
                    assert_eq!(key, threshold_key);
                    assert!(expected.contains("Critical"), "{expected}");
                }
                other => panic!("{threshold_key}={threshold}: expected a refusal, got {other:?}"),
            }
        }
    }
}

#[test]
fn ladder_accepts_threshold_below_critical() {
    assert!(validate_ladder("KEY", Decimal::new(5, 2), Decimal::new(2, 1)).is_ok());
}

#[test]
fn ladder_rejects_threshold_at_or_above_critical() {
    // Equal: Warning would be unreachable.
    assert!(validate_ladder("KEY", Decimal::new(2, 1), Decimal::new(2, 1)).is_err());
    // Above: every emitted signal would be Critical.
    assert!(validate_ladder("KEY", Decimal::new(3, 1), Decimal::new(2, 1)).is_err());
}
