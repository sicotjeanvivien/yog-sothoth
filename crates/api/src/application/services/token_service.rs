//! Application service for token queries.
//!
//! Assembles metadata and latest price for a single mint.
//! Returns `None` when the mint is unknown (no metadata row).

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    RepositoryError,
    domain::{TokenMetadata, TokenMetadataRepository, TokenPrice, TokenPriceRepository},
};

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

/// Token metadata combined with its optional latest price.
#[derive(Debug)]
pub(crate) struct TokenAggregate {
    pub metadata: TokenMetadata,
    pub price: Option<TokenPrice>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Application service for token queries.
pub(crate) struct TokenService {
    metadata_repo: Arc<dyn TokenMetadataRepository>,
    price_repo: Arc<dyn TokenPriceRepository>,
}

impl TokenService {
    pub(crate) fn new(
        metadata_repo: Arc<dyn TokenMetadataRepository>,
        price_repo: Arc<dyn TokenPriceRepository>,
    ) -> Self {
        Self {
            metadata_repo,
            price_repo,
        }
    }

    /// Fetch a token by mint address.
    ///
    /// Returns `Ok(None)` when no metadata row exists for this mint.
    /// Returns `Ok(Some(_))` with `price: None` when metadata exists
    /// but no price has been fetched yet.
    pub(crate) async fn get_token(
        &self,
        mint: &Pubkey,
    ) -> Result<Option<TokenAggregate>, RepositoryError> {
        let Some(metadata) = self.metadata_repo.find_by_mint(mint).await? else {
            return Ok(None);
        };

        let price = self.price_repo.find_latest_by_mint(mint).await?;

        Ok(Some(TokenAggregate { metadata, price }))
    }
}

#[cfg(test)]
#[path = "tests/token_service_tests.rs"]
mod tests;
