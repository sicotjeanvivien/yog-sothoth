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

/// What the source said about the mints it was asked for.
///
/// ⚠️ **Three cases, not two.** A mint is in `priced`, in `unpriced`, or in
/// **neither**: a mint whose request failed — a chunk given up on 429, an HTTP
/// or decoding error — was never answered, and putting it in `unpriced` would
/// tell `UnpricedMints` that the source has no price for a mint that may well
/// have one.
#[derive(Debug, Clone, Default)]
pub(crate) struct PriceAnswer {
    /// The mints the source returned a price for.
    pub(crate) priced: Vec<FetchedPrice>,
    /// The mints the source answered for without a price (untraded, flagged,
    /// not indexed yet…).
    pub(crate) unpriced: Vec<Pubkey>,
}

#[async_trait]
pub trait PriceSource: Send + Sync {
    /// Fetch USD prices for a batch of mints.
    ///
    /// Implementations must respect their own batch limit and absorb
    /// per-request failures: a failed request leaves its mints out of both
    /// lists of the [`PriceAnswer`], rather than failing the whole call.
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<PriceAnswer, SourceError>;
}
