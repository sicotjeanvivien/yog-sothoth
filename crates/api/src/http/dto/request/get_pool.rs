//! Request DTO for `GET /api/pools/{address}`.

use solana_pubkey::Pubkey;

use crate::http::{error::ApiError, query::parse_pool_address};

/// Validated input for the single-pool endpoint.
pub(crate) struct GetPoolRequest {
    pub(crate) pool_address: Pubkey,
}

impl GetPoolRequest {
    /// Parse and validate the path parameter.
    pub(crate) fn parse(address: String) -> Result<Self, ApiError> {
        Ok(Self {
            pool_address: parse_pool_address(&address)?,
        })
    }
}
