//! Request DTOs: `XxxRequest::parse(...)` runs every validation of one endpoint once, so a
//! value cannot exist invalid. The cursor codec lives in [`http::cursor`].
//!
//! [`http::cursor`]: crate::http::cursor

use std::str::FromStr;

use solana_pubkey::Pubkey;
use yog_core::domain::Protocol;

use crate::http::error::ApiError;

mod get_token;
mod list_announcements;
mod list_events;
mod list_liquidity_moves;
mod list_pools;
mod list_protocols;
mod list_rotation_runs;
mod list_signals;
mod list_token_activity;
mod list_token_prices;
mod list_tokens;
mod no_query;
mod window_param;

pub(crate) use get_token::GetTokenRequest;
pub(crate) use list_announcements::{AnnouncementsParams, ListAnnouncementsRequest};
pub(crate) use list_events::{EventsParams, ListEventsRequest};
pub(crate) use list_liquidity_moves::{LiquidityMovesParams, ListLiquidityMovesRequest};
pub(crate) use list_pools::{ListPoolsRequest, PoolsParams};
pub(crate) use list_protocols::{ListProtocolsRequest, ProtocolsParams};
pub(crate) use list_rotation_runs::{ListRotationRunsRequest, RotationRunsParams};
pub(crate) use list_signals::{ListSignalsRequest, SignalsParams};
pub(crate) use list_token_activity::{ListTokenActivityRequest, TokenActivityParams};
pub(crate) use list_token_prices::{ListTokenPricesRequest, TokenPricesParams};
pub(crate) use list_tokens::{ListTokensRequest, TokensParams};
pub(crate) use no_query::NoQuery;
pub(crate) use window_param::{
    MeasureActivityParams, MeasureWindowParams, ReadingSeriesParams, SeriesParams, WindowParams,
};

const DEFAULT_LIMIT: i64 = 50;
pub(crate) const MAX_LIMIT: i64 = 200;

fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

/// `limit`, or a 400 when it is outside `1..=MAX_LIMIT`.
fn validate_limit(limit: i64) -> Result<(), ApiError> {
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_LIMIT}, got {limit}"
        )));
    }
    Ok(())
}

/// `raw`, or a 400: a parameter present without a value is invalid, never absent.
fn non_empty<'a>(name: &str, raw: &'a str) -> Result<&'a str, ApiError> {
    if raw.is_empty() {
        return Err(ApiError::BadRequest(format!("`{name}` must not be empty")));
    }
    Ok(raw)
}

/// Parses a base58 address, or returns a 400 naming what it addresses (`pool`, `mint`).
fn parse_address(raw: &str, label: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw)
        .map_err(|_| ApiError::BadRequest(format!("invalid {label} address: {raw}")))
}

/// The `{protocol}` of a path, or a 400 naming it.
pub(crate) fn parse_protocol(raw: &str) -> Result<Protocol, ApiError> {
    Protocol::from_str(raw).map_err(|_| ApiError::BadRequest(format!("unknown protocol: {raw}")))
}

/// The `{pool}` of a path, or a 400.
pub(crate) fn parse_pool(raw: &str) -> Result<Pubkey, ApiError> {
    parse_address(raw, "pool")
}

/// The `{mint}` of a path, or a 400.
pub(crate) fn parse_mint(raw: &str) -> Result<Pubkey, ApiError> {
    parse_address(raw, "mint")
}
