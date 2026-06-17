//! Request DTO for `GET /api/pools/{address}/history`.

use solana_pubkey::Pubkey;

use crate::http::{
    error::ApiError,
    query::{HistoryQuery, parse_pool_address, validate_history_days},
};

/// Validated input for the pool history endpoint: a pool address and a
/// look-back window in days.
#[derive(Debug)]
pub(crate) struct GetPoolHistoryRequest {
    pub(crate) pool_address: Pubkey,
    /// Window in days, validated to `1..=MAX_HISTORY_DAYS`. `i32` to match the
    /// repository signature (bound straight into the SQL `make_interval`).
    pub(crate) days: i32,
}

impl GetPoolHistoryRequest {
    pub(crate) fn parse(address: String, query: HistoryQuery) -> Result<Self, ApiError> {
        let pool_address = parse_pool_address(&address)?;
        validate_history_days(query.days)?;
        Ok(Self {
            pool_address,
            // Safe cast: validated to 1..=90.
            days: query.days as i32,
        })
    }
}

#[cfg(test)]
#[path = "tests/get_pool_history_tests.rs"]
mod tests;
