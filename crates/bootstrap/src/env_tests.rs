use super::*;

// ⚠️ `set_var` is process-global and the harness parallel: every test uses unique key names.

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
fn required_trims_surrounding_whitespace() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_REQUIRED_CRLF", "postgresql://host/db\r\n");
        env::set_var("YOG_TEST_REQUIRED_U32_CRLF", " 10\r");
    }
    assert_eq!(
        required("YOG_TEST_REQUIRED_CRLF").unwrap(),
        "postgresql://host/db"
    );
    assert_eq!(
        parse_required_u32("YOG_TEST_REQUIRED_U32_CRLF").unwrap(),
        10
    );
}

#[test]
fn required_treats_a_blank_value_as_missing() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_REQUIRED_BLANK", "  \r\n");
    }
    let err = required("YOG_TEST_REQUIRED_BLANK").unwrap_err();
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

/// Mutation: the helper no longer lowercasing; implementors match lowercase only.
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

/// Mutation: no trim, and a CRLF `\r` refuses a value that looks right.
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
    // The exact fields: a crash log needs the value refused and the names
    // accepted.
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

#[test]
fn optional_trims_and_reads_a_blank_value_as_absent() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_OPTIONAL_PRESENT", "  pg_dump\r");
        env::set_var("YOG_TEST_OPTIONAL_BLANK", "  \r");
    }
    assert_eq!(
        optional("YOG_TEST_OPTIONAL_PRESENT").as_deref(),
        Some("pg_dump")
    );
    assert_eq!(optional("YOG_TEST_OPTIONAL_BLANK"), None);
    assert_eq!(optional("YOG_TEST_OPTIONAL_ABSENT"), None);
}

#[test]
fn parse_optional_falls_back_to_the_default_when_absent_or_blank() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_PARSE_OPTIONAL_PRESENT", " 600\r");
        env::set_var("YOG_TEST_PARSE_OPTIONAL_BLANK", "  \r");
    }
    let read = |key| parse_optional::<u32>(key, 60, "a count");
    assert_eq!(read("YOG_TEST_PARSE_OPTIONAL_PRESENT").unwrap(), 600);
    assert_eq!(read("YOG_TEST_PARSE_OPTIONAL_BLANK").unwrap(), 60);
    assert_eq!(read("YOG_TEST_PARSE_OPTIONAL_ABSENT").unwrap(), 60);
}

#[test]
fn parse_optional_refuses_what_the_type_refuses() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_PARSE_OPTIONAL_ZERO", "0");
    }
    let default = std::num::NonZeroU32::new(60).unwrap();
    let error = parse_optional("YOG_TEST_PARSE_OPTIONAL_ZERO", default, "at least 1")
        .expect_err("zero is not a NonZeroU32");

    assert!(
        matches!(
            &error,
            ConfigError::InvalidValue { key, value, expected }
                if key == "YOG_TEST_PARSE_OPTIONAL_ZERO" && value == "0" && *expected == "at least 1"
        ),
        "{error:?}"
    );
}

#[test]
fn optional_secret_url_reads_blank_and_absent_as_none() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_OPTIONAL_SECRET_URL_BLANK", " \r\n");
    }
    assert!(optional_secret_url("YOG_TEST_OPTIONAL_SECRET_URL_BLANK").is_none());
    assert!(optional_secret_url("YOG_TEST_OPTIONAL_SECRET_URL_ABSENT").is_none());
}

#[test]
fn optional_secret_url_wraps_a_present_value() {
    // SAFETY: unique key, isolated from other tests
    unsafe {
        env::set_var(
            "YOG_TEST_OPTIONAL_SECRET_URL_SET",
            "https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666",
        );
    }
    let url = optional_secret_url("YOG_TEST_OPTIONAL_SECRET_URL_SET").expect("set");
    assert_eq!(
        url.expose(),
        "https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666"
    );
    assert!(!format!("{url:?}").contains("0b7c5a1e"));
}
