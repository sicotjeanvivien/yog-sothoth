//! Request DTOs: `XxxRequest::parse(...)` runs every validation of one endpoint once, so a
//! value cannot exist invalid. The helpers live in [`http::query`] and [`http::cursor`].
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
