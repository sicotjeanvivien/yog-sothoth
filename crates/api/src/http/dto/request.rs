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
//! Validation helpers themselves live in `http::query` and
//! `http::cursor`. The request DTOs are their orchestrators, not
//! their replacement.

pub(crate) mod get_pool;
pub(crate) mod get_pool_latest_state;
pub(crate) mod get_token;
pub(crate) mod list_pool_liquidity;
pub(crate) mod list_pool_swaps;
pub(crate) mod list_pools;

pub(crate) use get_pool::GetPoolRequest;
pub(crate) use get_pool_latest_state::GetPoolLatestStateRequest;
pub(crate) use get_token::GetTokenRequest;
pub(crate) use list_pool_liquidity::ListPoolLiquidityRequest;
pub(crate) use list_pool_swaps::ListPoolSwapsRequest;
pub(crate) use list_pools::ListPoolsRequest;

#[cfg(test)]
#[path = "request/tests/common.rs"]
pub(super) mod test_common;
