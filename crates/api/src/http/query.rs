//! Query-parameter parsing, validation and normalization for the
//! pool endpoints. Pure HTTP-layer plumbing: translates raw query
//! strings into clean inputs, with no business logic.

use serde::Deserialize;
use solana_pubkey::Pubkey;
use std::str::FromStr;
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

pub(crate) const DEFAULT_HISTORY_DAYS: i64 = 7;
pub(crate) const MAX_HISTORY_DAYS: i64 = 90;

/// Query params for the pool history endpoint: a single `days` window.
#[derive(Debug, Deserialize)]
pub(crate) struct HistoryQuery {
    #[serde(default = "default_history_days")]
    pub(crate) days: i64,
}

pub(crate) fn default_history_days() -> i64 {
    DEFAULT_HISTORY_DAYS
}

/// Reject an out-of-range window rather than clamp it — a client asking for
/// `days=9999` has a bug worth surfacing, consistent with `validate_limit`.
pub(crate) fn validate_history_days(days: i64) -> Result<(), ApiError> {
    if !(1..=MAX_HISTORY_DAYS).contains(&days) {
        return Err(ApiError::BadRequest(format!(
            "days must be between 1 and {MAX_HISTORY_DAYS}"
        )));
    }
    Ok(())
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
    if let Some(c) = cursor
        && c.sort_column != sort.column()
    {
        return Err(ApiError::BadRequest(
            "cursor does not match the requested sort".to_string(),
        ));
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

/// Parse a base58 pool address from the path, or return a 400.
///
/// Centralised here because three endpoints share the same path
/// shape (`GET /api/pools/{address}`, `…/latest-state`, `…/swaps`,
/// `…/liquidity-events`). Keeping a single source of the error
/// message guarantees clients see a uniform string across all of
/// them.
pub(crate) fn parse_pool_address(raw: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw).map_err(|_| ApiError::BadRequest(format!("invalid pool address: {raw}")))
}

/// Parse a base58 SPL mint address from the path, or return a 400.
///
/// Mirrors `parse_pool_address` for the `/api/tokens/{mint}` route —
/// kept separate because the error message names the right concept
/// (mint vs pool address) which matters for client debugging.
pub(crate) fn parse_token_mint(raw: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw).map_err(|_| ApiError::BadRequest(format!("invalid mint address: {raw}")))
}

#[cfg(test)]
#[path = "tests/query_tests.rs"]
mod tests;
