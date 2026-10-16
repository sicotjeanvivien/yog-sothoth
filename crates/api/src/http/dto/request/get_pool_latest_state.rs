//! Request DTO for `GET /api/pools/{address}/latest-state`.

use solana_pubkey::Pubkey;

use crate::http::{error::ApiError, query::parse_pool_address};

/// Validated input for the latest-state endpoint: the address as a `Pubkey` and as the
/// `String` the projection is keyed by.
#[derive(Debug)]
pub(crate) struct GetPoolLatestStateRequest {
    #[allow(unused)]
    pub(crate) pool_address: Pubkey,
    pub(crate) raw_address: String,
}

impl GetPoolLatestStateRequest {
    pub(crate) fn parse(address: String) -> Result<Self, ApiError> {
        let pool_address = parse_pool_address(&address)?;
        Ok(Self {
            pool_address,
            raw_address: address,
        })
    }
}

#[cfg(test)]
#[path = "tests/get_pool_latest_state_tests.rs"]
mod tests;
