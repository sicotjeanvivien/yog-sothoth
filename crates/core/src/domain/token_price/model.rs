//! Token price domain model.
//!
//! A single USD price observation for an SPL mint, fetched from
//! Jupiter by the `yog-context` daemon. Pure domain type.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

/// Origin of a price observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceProvider {
    /// Fetched from the Jupiter price API.
    Jupiter,
    /// Fetched from Helius (DAS `price_info`).
    Helius,
    /// A last-known value reused because the live source was down.
    Fallback,
}

impl PriceProvider {
    /// Stable lowercase tag, as persisted in the `price_source`
    /// column.
    pub fn as_str(&self) -> &'static str {
        match self {
            PriceProvider::Jupiter => "jupiter",
            PriceProvider::Helius => "helius",
            PriceProvider::Fallback => "fallback",
        }
    }
}

/// A single USD price observation for a mint, at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenPrice {
    /// The SPL mint this price is for.
    ///
    /// A `Pubkey`, consistent with `TokenMetadata` and `Pool`.
    pub mint: Pubkey,

    /// Price in USD.
    ///
    /// `rust_decimal::Decimal` — an exact fixed-point decimal. Chosen
    /// over `f64` (lossy on very small memecoin values) and over a
    /// scaled `u128` (a price has no protocol-canonical scale factor,
    /// unlike on-chain sqrt_price/amounts). `rust_decimal` is a
    /// standalone crate with no tie to sqlx or Postgres, so it is
    /// safe for `core` to depend on — the persistence layer maps it
    /// to the `NUMERIC` column.
    pub price_usd: Decimal,

    /// Which source produced this price.
    pub price_provider: PriceProvider,

    /// Optional confidence value, when the source provides one.
    pub confidence: Option<f32>,

    /// When the price was fetched.
    pub fetched_at: DateTime<Utc>,
}
