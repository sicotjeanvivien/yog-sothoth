//! Application service for token queries.
//!
//! Assembles metadata and latest price for a single mint.
//! Returns `None` when the mint is unknown (no metadata row).

use std::sync::Arc;

use chrono::Utc;
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryError, RepositoryResult,
    domain::{
        TokenCursor, TokenEntry, TokenMetadata, TokenMetadataLookup, TokenPrice, TokenPriceCursor,
        TokenPriceLookup, Window,
    },
};

/// Token metadata combined with its optional latest price.
#[derive(Debug)]
pub(crate) struct TokenAggregate {
    pub metadata: TokenMetadata,
    pub price: Option<TokenPrice>,
}

/// Application service for token queries.
pub(crate) struct TokenService {
    metadata_repo: Arc<dyn TokenMetadataLookup>,
    price_repo: Arc<dyn TokenPriceLookup>,
}

impl TokenService {
    pub(crate) fn new(
        metadata_repo: Arc<dyn TokenMetadataLookup>,
        price_repo: Arc<dyn TokenPriceLookup>,
    ) -> Self {
        Self {
            metadata_repo,
            price_repo,
        }
    }

    /// A token by mint: `None` without a metadata row, `price: None` before its first price.
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

    /// The metadata of `mint`, `None` before its first fetch: a token the catalog knows.
    pub(crate) async fn find_metadata(
        &self,
        mint: &Pubkey,
    ) -> RepositoryResult<Option<TokenMetadata>> {
        self.metadata_repo.find_by_mint(mint).await
    }

    pub(crate) async fn list_tokens(
        &self,
        cursor: Option<TokenCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<TokenEntry>> {
        self.metadata_repo.list_entries(cursor, limit).await
    }

    /// `mint`'s prices over `window`, ending now on the first page; a later page keeps the
    /// start its cursor carries, so the window does not slide under the pages.
    pub(crate) async fn list_token_prices(
        &self,
        mint: &Pubkey,
        window: Window,
        cursor: Option<TokenPriceCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<TokenPrice>> {
        let from = cursor
            .as_ref()
            .map_or_else(|| Utc::now() - window.duration(), |c| c.from);
        self.price_repo.list_prices(mint, from, cursor, limit).await
    }
}

#[cfg(test)]
#[path = "tests/token_service_tests.rs"]
mod tests;
