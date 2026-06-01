use crate::http::query::{
    MAX_LIMIT, MAX_SEARCH_LEN, PageDirectionParam, PagePositionParam, PageQuery, PoolSortParam,
    normalize_search, validate_limit, validate_pagination_query, validate_search,
};

#[test]
fn normalize_search_trims_and_collapses_blank() {
    assert_eq!(normalize_search(None), None);
    assert_eq!(normalize_search(Some("".into())), None);
    assert_eq!(normalize_search(Some("   ".into())), None);
    assert_eq!(normalize_search(Some("  SOL ".into())), Some("SOL".into()));
    assert_eq!(normalize_search(Some("BONK".into())), Some("BONK".into()));
}

#[test]
fn validate_search_rejects_overlong() {
    let long = "x".repeat(MAX_SEARCH_LEN + 1);
    assert!(validate_search(Some(&long)).is_err());

    let ok = "x".repeat(MAX_SEARCH_LEN);
    assert!(validate_search(Some(&ok)).is_ok());
    assert!(validate_search(None).is_ok());
}

#[test]
fn validate_limit_bounds() {
    assert!(validate_limit(0).is_err());
    assert!(validate_limit(1).is_ok());
    assert!(validate_limit(MAX_LIMIT).is_ok());
    assert!(validate_limit(MAX_LIMIT + 1).is_err());
}

#[test]
fn validate_pagination_rejects_cursor_with_position() {
    let q = PageQuery {
        cursor: Some("x".into()),
        dir: PageDirectionParam::Next,
        sort: PoolSortParam::FirstSeenAsc,
        position: Some(PagePositionParam::Last),
        q: None,
        limit: 50,
    };
    assert!(validate_pagination_query(&q).is_err());
}

#[test]
fn validate_pagination_allows_cursor_alone() {
    let q = PageQuery {
        cursor: Some("x".into()),
        dir: PageDirectionParam::Next,
        sort: PoolSortParam::FirstSeenAsc,
        position: None,
        q: None,
        limit: 50,
    };
    assert!(validate_pagination_query(&q).is_ok());
}
