//! Persistence contracts of the `token_prices` hypertable.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::{
    Page, RepositoryResult,
    domain::{TokenPrice, TokenPriceCursor},
};

/// Write side of token prices, yog-context's lens.
#[async_trait]
pub trait TokenPriceRepository: Send + Sync {
    /// Append a batch of price observations in one statement.
    /// ⚠️ The caller filters on [`TokenPrice::is_storable`]: one unstorable row
    /// fails the batch, and the lost tick is never backfilled.
    async fn insert_batch(&self, prices: &[TokenPrice]) -> RepositoryResult<()>;
}

/// Latest-price reads, the api's lens.
#[async_trait]
pub trait TokenPriceLookup: Send + Sync {
    /// The latest price of a mint, `None` if never priced.
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>>;

    /// A page of `mint`'s prices fetched from `from` on, the most recent first, after
    /// `cursor`'s key; forward only. `cursor` is of `mint` and `from`.
    async fn list_prices(
        &self,
        mint: &Pubkey,
        from: DateTime<Utc>,
        cursor: Option<TokenPriceCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<TokenPrice>>;
}
