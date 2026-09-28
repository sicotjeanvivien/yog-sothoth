//! Request DTOs — typed, validated-by-construction inputs to the
//! HTTP handlers.
//!
//! Each request DTO encapsulates the full validation pipeline for one
//! endpoint:
//!
//!   - serde-deserialized extractors (path + query) come in as raw,
//!   - `XxxRequest::parse(...)` runs every validation rule once,
//!   - the resulting value is impossible to construct in an invalid
//!     state; downstream code (services, mappers) can rely on its
//!     fields unconditionally.
//!
//! Validation helpers themselves live in [`http::query`] and
//! [`http::cursor`]. The request DTOs are their orchestrators, not
//! their replacement.
//!
//! [`http::query`]: crate::http::query
//! [`http::cursor`]: crate::http::cursor

mod get_pool;
mod get_pool_history;
mod get_pool_latest_state;
mod get_token;
mod list_pool_liquidity;
mod list_pool_swaps;
mod list_pools;
mod list_signals;
mod list_top_pools;

pub(crate) use get_pool::GetPoolRequest;
pub(crate) use get_pool_history::GetPoolHistoryRequest;
pub(crate) use get_pool_latest_state::GetPoolLatestStateRequest;
pub(crate) use get_token::GetTokenRequest;
pub(crate) use list_pool_liquidity::ListPoolLiquidityRequest;
pub(crate) use list_pool_swaps::ListPoolSwapsRequest;
pub(crate) use list_pools::ListPoolsRequest;
pub(crate) use list_signals::ListSignalsRequest;
pub(crate) use list_top_pools::ListTopPoolsRequest;

#[cfg(test)]
#[path = "request/tests/common.rs"]
pub(super) mod test_common;
