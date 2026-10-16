//! A USD price observation for an SPL mint, fetched by `yog-context`.

use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use solana_pubkey::Pubkey;

use crate::CoreError;

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
    /// Stable tag of the `price_source` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            PriceProvider::Jupiter => "jupiter",
            PriceProvider::Helius => "helius",
            PriceProvider::Fallback => "fallback",
        }
    }
}

impl std::str::FromStr for PriceProvider {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jupiter" => Ok(PriceProvider::Jupiter),
            "helius" => Ok(PriceProvider::Helius),
            "fallback" => Ok(PriceProvider::Fallback),
            _ => Err(CoreError::UnknownProgram(s.to_string())),
        }
    }
}

/// A single USD price observation for a mint, at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenPrice {
    /// The SPL mint this price is for.
    pub mint: Pubkey,

    /// Price in USD.
    pub price_usd: Decimal,

    /// Which source produced this price.
    pub price_provider: PriceProvider,

    /// Optional confidence value, when the source provides one.
    pub confidence: Option<f32>,

    /// When the price was fetched.
    pub fetched_at: DateTime<Utc>,
}

/// Scale of the `token_prices.price_usd` column (`NUMERIC(38, 18)`).
/// ⚠️ A mirror of the schema: change it with the column, or `is_storable` lies.
pub const PRICE_STORAGE_SCALE: u32 = 18;

/// Precision of the same column: with the scale, a value must round below `10^20`.
pub const PRICE_STORAGE_PRECISION: u32 = 38;

/// The exclusive upper bound of [`TokenPrice::is_storable`]: `10^20`.
fn storage_ceiling() -> Decimal {
    Decimal::from_i128_with_scale(10i128.pow(PRICE_STORAGE_PRECISION - PRICE_STORAGE_SCALE), 0)
}

impl TokenPrice {
    /// Whether `token_prices.price_usd` can hold this price, rounded as Postgres rounds.
    /// ⚠️ Not `> 0`: below `5e-19` the value rounds to a zero the CHECK refuses, at
    /// `10^20` the type refuses it, and either fails the whole batch insert.
    pub fn is_storable(&self) -> bool {
        let rounded = self
            .price_usd
            .round_dp_with_strategy(PRICE_STORAGE_SCALE, RoundingStrategy::MidpointAwayFromZero);

        rounded > Decimal::ZERO && rounded < storage_ceiling()
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
