//! Query-parameter parsing, validation and normalization for the
//! pool endpoints. Pure HTTP-layer plumbing: translates raw query
//! strings into clean inputs, with no business logic.

use rust_decimal::Decimal;
use serde::Deserialize;
use solana_pubkey::Pubkey;
use std::str::FromStr;
use yog_core::{
    PageDirection, PagePosition, PoolSort,
    domain::{PoolCursor, PoolRankMetric, Severity},
};

use crate::http::error::ApiError;

pub(crate) const DEFAULT_LIMIT: i64 = 50;
pub(crate) const MAX_LIMIT: i64 = 200;
pub(crate) const MAX_SEARCH_LEN: usize = 100;

pub(crate) const DEFAULT_TOP_LIMIT: i64 = 10;
pub(crate) const MAX_TOP_LIMIT: i64 = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct PageQuery {
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) dir: PageDirectionParam,
    pub(crate) position: Option<PagePositionParam>,
    #[serde(default)]
    pub(crate) sort: PoolSortParam,
    pub(crate) q: Option<String>,
    /// Exact base-fee tier filter (basis points), as a decimal string on the
    /// wire (e.g. `fee_bps=25`). Parsed and validated in the request DTO.
    pub(crate) fee_bps: Option<String>,
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

/// Query params for `GET /api/signals`: standard cursor pagination plus
/// optional exact severity and pool filters. No sort (the feed ordering
/// is fixed by contract) and no free-text search.
#[derive(Debug, Deserialize)]
pub(crate) struct SignalsQuery {
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) dir: PageDirectionParam,
    pub(crate) position: Option<PagePositionParam>,
    pub(crate) severity: Option<SeverityParam>,
    pub(crate) pool: Option<String>,
    #[serde(default = "default_limit")]
    pub(crate) limit: i64,
}

/// Wire form of the severity filter. An unknown value fails serde
/// deserialization → axum returns 400 before the handler runs.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SeverityParam {
    Info,
    Warning,
    Critical,
}

impl From<SeverityParam> for Severity {
    fn from(value: SeverityParam) -> Self {
        match value {
            SeverityParam::Info => Severity::Info,
            SeverityParam::Warning => Severity::Warning,
            SeverityParam::Critical => Severity::Critical,
        }
    }
}

/// Query params for `GET /api/pools/top`: the ranking metric and how many
/// rows. Non-paginated — a small capped ranking, not a navigable list.
#[derive(Debug, Deserialize)]
pub(crate) struct TopPoolsQuery {
    #[serde(default)]
    pub(crate) metric: PoolRankMetricParam,
    #[serde(default = "default_top_limit")]
    pub(crate) limit: i64,
}

pub(crate) fn default_top_limit() -> i64 {
    DEFAULT_TOP_LIMIT
}

/// Wire form of the ranking metric. An unknown value fails serde
/// deserialization → axum returns 400 before the handler runs.
#[derive(Debug, Default, Deserialize, Clone, Copy)]
pub(crate) enum PoolRankMetricParam {
    #[default]
    #[serde(rename = "volume_24h")]
    Volume24h,
    #[serde(rename = "tvl")]
    Tvl,
}

impl From<PoolRankMetricParam> for PoolRankMetric {
    fn from(value: PoolRankMetricParam) -> Self {
        match value {
            PoolRankMetricParam::Volume24h => PoolRankMetric::Volume24h,
            PoolRankMetricParam::Tvl => PoolRankMetric::Tvl,
        }
    }
}

/// Validate the top-N `limit` against its (smaller) accepted range.
pub(crate) fn validate_top_limit(limit: i64) -> Result<(), ApiError> {
    if !(1..=MAX_TOP_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_TOP_LIMIT}, got {limit}"
        )));
    }
    Ok(())
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
    validate_cursor_position_exclusive(query.cursor.is_some(), query.position.is_some())
}

/// The rule behind [`validate_pagination_query`], shared with query
/// types that don't use `PageQuery` (e.g. `SignalsQuery`).
pub(crate) fn validate_cursor_position_exclusive(
    has_cursor: bool,
    has_position: bool,
) -> Result<(), ApiError> {
    if has_cursor && has_position {
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

/// Parse the optional `fee_bps` filter into a domain `Decimal`.
///
/// A blank value collapses to `None` (no filter) — same tolerance as the
/// search term, so an empty `?fee_bps=` from the UI clearing the selection
/// is a valid "unfiltered" request, not a 400. A non-numeric or negative
/// value is a client bug worth surfacing (a fee tier is never negative).
pub(crate) fn parse_fee_bps(raw: Option<String>) -> Result<Option<Decimal>, ApiError> {
    let Some(trimmed) = raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    let value = Decimal::from_str(&trimmed)
        .map_err(|_| ApiError::BadRequest(format!("`fee_bps` must be a number, got {trimmed}")))?;

    if value.is_sign_negative() {
        return Err(ApiError::BadRequest(
            "`fee_bps` must not be negative".to_string(),
        ));
    }

    Ok(Some(value))
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
