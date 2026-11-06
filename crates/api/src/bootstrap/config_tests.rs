use super::{parse_cors_origins, parse_metrics_addr};
use yog_bootstrap::ConfigError;

#[test]
fn parses_a_single_origin() {
    let origins = parse_cors_origins("https://yog-scope.xyz").unwrap();
    assert_eq!(origins, ["https://yog-scope.xyz"]);
}

#[test]
fn parses_a_comma_separated_list_trimming_whitespace() {
    let origins = parse_cors_origins("http://localhost:3000 , https://yog-scope.xyz").unwrap();
    assert_eq!(origins, ["http://localhost:3000", "https://yog-scope.xyz"]);
}

#[test]
fn skips_empty_entries_from_a_trailing_comma() {
    let origins = parse_cors_origins("https://yog-scope.xyz,").unwrap();
    assert_eq!(origins, ["https://yog-scope.xyz"]);
}

#[test]
fn rejects_an_effectively_empty_list() {
    let err = parse_cors_origins("  , ").unwrap_err();
    assert!(matches!(
        err,
        ConfigError::InvalidValue { ref key, .. } if key == "API_CORS_ALLOWED_ORIGINS"
    ));
}

#[test]
fn rejects_an_origin_with_invalid_header_bytes() {
    // A NUL byte mid-string is not a legal header value byte, and
    // (unlike surrounding whitespace) survives the per-entry trim.
    let err = parse_cors_origins("https://yog-\u{0}scope.xyz").unwrap_err();
    assert!(matches!(
        err,
        ConfigError::InvalidValue { ref key, .. } if key == "API_CORS_ALLOWED_ORIGINS"
    ));
}

/// Mutation: the default moved off the daemons' port, which `observe.sh` reads.
#[test]
fn the_metrics_listen_on_the_daemons_port_by_default() {
    assert_eq!(
        parse_metrics_addr(None).unwrap().to_string(),
        "0.0.0.0:9000"
    );
}

#[test]
fn the_metrics_listen_where_they_are_told() {
    let addr = parse_metrics_addr(Some("127.0.0.1:9004".to_string())).unwrap();
    assert_eq!(addr.to_string(), "127.0.0.1:9004");
}

#[test]
fn rejects_a_metrics_address_that_is_no_socket_address() {
    let err = parse_metrics_addr(Some("9004".to_string())).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::InvalidValue { ref key, .. } if key == "API_METRICS_ADDR"
    ));
}
