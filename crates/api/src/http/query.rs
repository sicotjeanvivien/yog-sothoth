//! Query-parameter parsing and validation. Pure HTTP-layer plumbing:
//! translates raw query strings into clean inputs, with no business logic.

use serde::Deserialize;
use solana_pubkey::Pubkey;
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};
use yog_core::{PageDirection, PagePosition, domain::Severity};

use crate::http::error::ApiError;

pub(crate) const DEFAULT_LIMIT: i64 = 50;
pub(crate) const MAX_LIMIT: i64 = 200;

pub(crate) fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
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

#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
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

/// Query params for `GET /api/signals`: standard cursor pagination plus
/// optional exact severity and pool filters. No sort (the feed ordering
/// is fixed by contract) and no free-text search.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct SignalsQuery {
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    #[param(inline)]
    pub(crate) dir: PageDirectionParam,
    #[param(inline)]
    pub(crate) position: Option<PagePositionParam>,
    #[param(inline)]
    pub(crate) severity: Option<SeverityParam>,
    pub(crate) pool: Option<String>,
    #[serde(default = "default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    pub(crate) limit: i64,
}

/// Wire form of the severity filter. An unknown value fails serde
/// deserialization → axum returns 400 before the handler runs.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
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

/// Parses a base58 pool address, or returns a 400.
pub(crate) fn parse_pool_address(raw: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw).map_err(|_| ApiError::BadRequest(format!("invalid pool address: {raw}")))
}

/// Parses a base58 mint address from the path, or returns a 400 naming a mint.
pub(crate) fn parse_token_mint(raw: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw).map_err(|_| ApiError::BadRequest(format!("invalid mint address: {raw}")))
}

#[cfg(test)]
#[path = "tests/query_tests.rs"]
mod tests;
