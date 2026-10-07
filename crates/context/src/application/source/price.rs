use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::domain::PriceProvider;

use crate::error::SourceError;

/// A fetched price, which the worker turns into a `TokenPrice`.
#[derive(Debug, Clone)]
pub(crate) struct FetchedPrice {
    pub(crate) mint: Pubkey,
    pub(crate) price_provider: PriceProvider,
    pub(crate) price_usd: Decimal,
}

/// What the source said about the mints it was asked for.
///
/// ⚠️ A mint whose request failed is in **neither** list: it was never
/// answered.
#[derive(Debug, Clone, Default)]
pub(crate) struct PriceAnswer {
    /// The mints the source returned a price for.
    pub(crate) priced: Vec<FetchedPrice>,
    /// The mints the source answered for without a price.
    pub(crate) unpriced: Vec<Pubkey>,
}

#[async_trait]
pub trait PriceSource: Send + Sync {
    /// Fetch USD prices for a batch of mints. A failed request leaves its mints
    /// out of the [`PriceAnswer`] rather than failing the call.
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<PriceAnswer, SourceError>;
}
