use crate::http::query::{MAX_LIMIT, validate_cursor_position_exclusive, validate_limit};

#[test]
fn validate_limit_bounds() {
    assert!(validate_limit(0).is_err());
    assert!(validate_limit(1).is_ok());
    assert!(validate_limit(MAX_LIMIT).is_ok());
    assert!(validate_limit(MAX_LIMIT + 1).is_err());
}

#[test]
fn validate_pagination_rejects_cursor_with_position() {
    assert!(validate_cursor_position_exclusive(true, true).is_err());
}

#[test]
fn validate_pagination_allows_cursor_alone() {
    assert!(validate_cursor_position_exclusive(true, false).is_ok());
}
