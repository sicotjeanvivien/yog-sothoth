//! Request DTO for `GET /api/pools/{address}/latest-state`.
//!
//! Distinct from `GetPoolRequest` despite carrying the same field:
//! either endpoint may grow its own params over time (e.g. a future
//! `?at=<timestamp>` for historical snapshots) without dragging the
//! other along.

use solana_pubkey::Pubkey;

use crate::http::{error::ApiError, query::parse_pool_address};

/// Validated input for the latest-state endpoint.
///
/// Carries the address both as a `Pubkey` (for downstream typed use)
/// and as a `String` (the projection table is keyed by string in
/// persistence, and the service signature matches that).
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
