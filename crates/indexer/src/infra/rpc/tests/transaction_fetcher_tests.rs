use super::*;

#[test]
fn rate_limit_variants_are_classified() {
    for raw in [
        "HTTP 429 Too Many Requests",
        "rate limit exceeded",
        "Too Many Requests",
    ] {
        let e = FetchError::from_rpc_string(raw.to_string());
        assert!(matches!(e, FetchError::RateLimited), "raw = {raw:?}");
        assert_eq!(e.metric_label(), "rate_limited");
    }
}

#[test]
fn timeout_is_classified() {
    let e = FetchError::from_rpc_string("request timed out".to_string());
    assert!(matches!(e, FetchError::Timeout));
    assert_eq!(e.metric_label(), "timeout");
}

#[test]
fn null_response_maps_to_not_found() {
    let e = FetchError::from_rpc_string("got null in response".to_string());
    assert!(matches!(e, FetchError::NotFound));
    assert_eq!(e.metric_label(), "not_found");
}

#[test]
fn unknown_falls_back_to_other() {
    let e = FetchError::from_rpc_string("some unexpected RPC error".to_string());
    assert!(matches!(e, FetchError::Other(_)));
    assert_eq!(e.metric_label(), "other");
}

#[test]
fn connection_keyword_is_classified() {
    let e = FetchError::from_rpc_string("connection refused".to_string());
    assert!(matches!(e, FetchError::Connection(_)));
    assert_eq!(e.metric_label(), "connection_error");
}
