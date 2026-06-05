use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::domain::PriceProvider;

use crate::error::SourceError;

/// A successfully fetched price, ready to be turned into the domain
/// `TokenPrice` by the worker.
#[derive(Debug, Clone)]
pub(crate) struct FetchedPrice {
    pub(crate) mint: Pubkey,
    pub(crate) price_provider: PriceProvider,
    pub(crate) price_usd: Decimal,
}

#[async_trait]
pub trait PriceSource: Send + Sync {
    /// Fetch USD prices for a batch of mints.
    ///
    /// Implementations must respect their own batch limit (the worker
    /// chunks the queue before calling). The returned `Vec` contains
    /// only the mints the source could actually price — mints the
    /// source cannot price (untraded, flagged, etc.) are silently
    /// dropped.
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError>;
}
