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

// ── parse_required_enum ──────────────────────────────────────────────
//
// A stand-in for a real config enum: what is under test is the helper's
// contract — lowercasing, and the shape of the refusal — not any
// particular set of names.

#[derive(Debug, PartialEq, Eq)]
enum Colour {
    Red,
    Blue,
}

impl EnvEnum for Colour {
    const EXPECTED: &'static str = "red or blue";

    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "red" => Some(Self::Red),
            "blue" => Some(Self::Blue),
            _ => None,
        }
    }
}

#[test]
fn parse_required_enum_accepts_a_known_name() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_ENUM_KNOWN", "blue");
    }
    assert_eq!(
        parse_required_enum::<Colour>("YOG_TEST_ENUM_KNOWN").unwrap(),
        Colour::Blue
    );
}

/// The one that pins case-insensitivity to the *helper*. Implementors
/// match on lowercase literals only, so if `parse_required_enum` ever
/// stops lowercasing, every enum in the workspace silently starts
/// rejecting values an operator would reasonably type.
#[test]
fn parse_required_enum_ignores_case() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_ENUM_UPPER", "RED");
        env::set_var("YOG_TEST_ENUM_MIXED", "BlUe");
    }
    assert_eq!(
        parse_required_enum::<Colour>("YOG_TEST_ENUM_UPPER").unwrap(),
        Colour::Red
    );
    assert_eq!(
        parse_required_enum::<Colour>("YOG_TEST_ENUM_MIXED").unwrap(),
        Colour::Blue
    );
}

/// Same reason as the case test, and less obvious: this repository's
/// `.env` has CRLF line endings. A `\r` reaching `from_env_value` would
/// be refused with a message showing a value that looks correct.
#[test]
fn parse_required_enum_ignores_surrounding_whitespace() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_ENUM_PADDED", "  Blue\r\n");
    }
    assert_eq!(
        parse_required_enum::<Colour>("YOG_TEST_ENUM_PADDED").unwrap(),
        Colour::Blue
    );
}

#[test]
fn parse_required_enum_rejects_an_unknown_name_and_lists_the_accepted_ones() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_ENUM_UNKNOWN", "green");
    }
    let err = parse_required_enum::<Colour>("YOG_TEST_ENUM_UNKNOWN").unwrap_err();
    // Asserting the exact fields, not just the variant: an error that
    // does not echo the value it refused and the names it wanted is of
    // no use to whoever reads the crash log.
    match err {
        ConfigError::InvalidValue {
            key,
            value,
            expected,
        } => {
            assert_eq!(key, "YOG_TEST_ENUM_UNKNOWN");
            assert_eq!(value, "green");
            assert_eq!(expected, "red or blue");
        }
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn parse_required_enum_fails_when_absent() {
    let err = parse_required_enum::<Colour>("YOG_TEST_ENUM_ABSENT").unwrap_err();
    assert!(matches!(err, ConfigError::MissingVariable(_)));
}
