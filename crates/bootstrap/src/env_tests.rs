use super::*;

// SAFETY-NOTE on env tests: tests run in parallel by default, and
// `env::set_var` is process-global. Tests that mutate the
// environment must use unique key names to avoid interfering with
// each other.

#[test]
fn required_returns_value_when_present() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_REQUIRED_PRESENT", "value");
    }
    assert_eq!(required("YOG_TEST_REQUIRED_PRESENT").unwrap(), "value");
}

#[test]
fn required_fails_when_absent() {
    let err = required("YOG_TEST_REQUIRED_ABSENT").unwrap_err();
    assert!(matches!(err, ConfigError::MissingVariable(_)));
}

#[test]
fn required_fails_when_empty() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_REQUIRED_EMPTY", "");
    }
    let err = required("YOG_TEST_REQUIRED_EMPTY").unwrap_err();
    assert!(matches!(err, ConfigError::MissingVariable(_)));
}

#[test]
fn parse_required_bool_accepts_true_false_case_insensitive() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_BOOL_T", "TRUE");
        env::set_var("YOG_TEST_BOOL_F", "False");
    }
    assert!(parse_required_bool("YOG_TEST_BOOL_T").unwrap());
    assert!(!parse_required_bool("YOG_TEST_BOOL_F").unwrap());
}

#[test]
fn parse_required_bool_rejects_garbage() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_BOOL_BAD", "yes");
    }
    let err = parse_required_bool("YOG_TEST_BOOL_BAD").unwrap_err();
    assert!(matches!(err, ConfigError::InvalidValue { .. }));
}
