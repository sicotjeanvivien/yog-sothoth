//! Query-parameter parsing, validation and normalization for the
//! pool endpoints. Pure HTTP-layer plumbing: translates raw query
//! strings into clean inputs, with no business logic.

use serde::Deserialize;
use yog_core::{PageDirection, PagePosition, PoolSort, domain::PoolCursor};

use crate::http::error::ApiError;

pub(crate) const DEFAULT_LIMIT: i64 = 50;
pub(crate) const MAX_LIMIT: i64 = 200;
pub(crate) const MAX_SEARCH_LEN: usize = 100;

#[derive(Debug, Deserialize)]
pub(crate) struct PageQuery {
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) dir: PageDirectionParam,
    pub(crate) position: Option<PagePositionParam>,
    #[serde(default)]
    pub(crate) sort: PoolSortParam,
    pub(crate) q: Option<String>,
    #[serde(default = "default_limit")]
    pub(crate) limit: i64,
}

pub(crate) fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

#[derive(Debug, Default, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PageDirectionParam {
    #[default]
    Next,
    Prev,
}

impl From<PageDirectionParam> for PageDirection {
    fn from(value: PageDirectionParam) -> Self {
        match value {
            PageDirectionParam::Next => PageDirection::Next,
            PageDirectionParam::Prev => PageDirection::Prev,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PagePositionParam {
    First,
    Last,
}

impl From<PagePositionParam> for PagePosition {
    fn from(value: PagePositionParam) -> Self {
        match value {
            PagePositionParam::First => PagePosition::First,
            PagePositionParam::Last => PagePosition::Last,
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PoolSortParam {
    #[default]
    FirstSeenDesc,
    FirstSeenAsc,
    LastSeenDesc,
    LastSeenAsc,
}

impl From<PoolSortParam> for PoolSort {
    fn from(value: PoolSortParam) -> Self {
        match value {
            PoolSortParam::FirstSeenDesc => PoolSort::FirstSeenDesc,
            PoolSortParam::FirstSeenAsc => PoolSort::FirstSeenAsc,
            PoolSortParam::LastSeenDesc => PoolSort::LastSeenDesc,
            PoolSortParam::LastSeenAsc => PoolSort::LastSeenAsc,
        }
    }
}

/// Validate the `limit` query param against the accepted range.
pub(crate) fn validate_limit(limit: i64) -> Result<(), ApiError> {
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_LIMIT}, got {limit}"
        )));
    }
    Ok(())
}

/// Reject `position` combined with `cursor` (contradictory directives).
pub(crate) fn validate_pagination_query(query: &PageQuery) -> Result<(), ApiError> {
    if query.position.is_some() && query.cursor.is_some() {
        return Err(ApiError::BadRequest(
            "`position` cannot be combined with `cursor`".to_string(),
        ));
    }
    Ok(())
}

/// Reject an over-long search term (cheap DoS guard on `ILIKE`).
pub(crate) fn validate_search(q: Option<&str>) -> Result<(), ApiError> {
    if let Some(raw) = q
        && raw.chars().count() > MAX_SEARCH_LEN
    {
        return Err(ApiError::BadRequest(format!(
            "`q` must be at most {MAX_SEARCH_LEN} characters"
        )));
    }
    Ok(())
}

/// Reject a cursor whose embedded sort column disagrees with the
/// active `sort` param. This catches a tampered or stale URL where a
/// cursor built under one sort is replayed under another — which
/// would otherwise produce a silently wrong page.
pub(crate) fn validate_cursor_sort_consistency(
    cursor: Option<&PoolCursor>,
    sort: PoolSort,
) -> Result<(), ApiError> {
    if let Some(c) = cursor {
        if c.sort_column != sort.column() {
            return Err(ApiError::BadRequest(
                "cursor does not match the requested sort".to_string(),
            ));
        }
    }
    Ok(())
}

/// Normalize a raw search term into a clean optional value: trim
/// surrounding whitespace, collapse blank to `None`. The repository
/// must never receive a blank string (it would match everything via
/// `%%`).
pub(crate) fn normalize_search(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
