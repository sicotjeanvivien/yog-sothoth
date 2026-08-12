//! Token price domain model.
//!
//! A single USD price observation for an SPL mint, fetched from
//! Jupiter by the `yog-context` daemon. Pure domain type.

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

/// Decimal scale of the `token_prices.price_usd` column
/// (`NUMERIC(38, 18)`).
///
/// A mirror of the schema, not a preference: it is the number of decimals a
/// price actually keeps once stored, and therefore the only scale at which
/// "is this price zero?" can be asked truthfully. Changing the column's scale
/// without changing this constant makes [`TokenPrice::is_storable`] lie —
/// `price_positivity.rs` (persistence integration tests) is what catches that.
pub const PRICE_STORAGE_SCALE: u32 = 18;

/// Total precision of the same column — the `38` in `NUMERIC(38, 18)`.
///
/// With [`PRICE_STORAGE_SCALE`] it fixes the *upper* end of what the column can
/// hold: `38 - 18 = 20` integer digits, so a value must round to strictly less
/// than `10^20`. Postgres words it exactly that way when it refuses one:
/// *"A field with precision 38, scale 18 must round to an absolute value less
/// than 10^20."*
pub const PRICE_STORAGE_PRECISION: u32 = 38;

/// The exclusive upper bound of [`TokenPrice::is_storable`]: `10^20`.
fn storage_ceiling() -> Decimal {
    Decimal::from_i128_with_scale(10i128.pow(PRICE_STORAGE_PRECISION - PRICE_STORAGE_SCALE), 0)
}

impl TokenPrice {
    /// Whether `token_prices.price_usd` can actually hold this price.
    ///
    /// **Both ends of the column matter, and they fail differently.**
    ///
    /// ## The low end — why this is not `price_usd > 0`
    ///
    /// The column keeps [`PRICE_STORAGE_SCALE`] decimals, so **any** value below
    /// `5e-19` is rounded to exactly `0` on write — while being perfectly
    /// positive in Rust, and positive in SQL right up until the coercion. A
    /// `> 0` test passes it through and the row lands as a zero.
    ///
    /// That zero is the whole problem: an absent price is `NULL`, which
    /// annihilates every product it takes part in and is caught by the
    /// `valuation_complete` guards; a zero *multiplies*, yields a plausible
    /// number, and those guards — which ask `price_usd IS NULL` — wave it
    /// through.
    ///
    /// ## How reachable that is — through the wire, not through the market
    ///
    /// This doc used to close the paragraph above by claiming that
    /// very-high-supply memecoins live in exactly this regime. **That is
    /// false**, and the same claim is frozen into two migrations that
    /// forward-only forbids editing, in two different wordings —
    /// `009_price_positivity.sql:19` (*"live in exactly that regime"*) and
    /// `002_swap_implied_price.sql:241-243` (*"live in precisely that range —
    /// which is to say, the population this migration exists to rescue"*).
    /// Neither is quotable as a single string; the correction for both lives in
    /// `crates/persistence/migrations/README.md`.
    ///
    /// A mint's `decimals` bounds *amounts*, not prices: a price is a ratio and
    /// has no on-chain quantum. What bounds it below is supply. Whole-token
    /// supply is **at most** `u64::MAX / 10^decimals`, so a price under `5e-19`
    /// implies a total valuation under `5e-19 × 1.8447e19 ≈ $9.22` — and that is
    /// the absolute extreme, at `decimals = 0`. At the pump.fun standard of 6
    /// the same bound is `$9.2e-6`.
    ///
    /// Measured against live Jupiter data on 12 August 2026, over the 205 mints
    /// returned by its search and recent-launch endpoints: the floor of the real
    /// population is ~`1e-10` (BabyDoge, `decimals = 1`, supply `2.96e17`, price
    /// `3.47e-10`, $116k liquidity, FDV $102M) — nine orders of magnitude above
    /// the cliff. Reaching it would take that token's FDV to $0.15, and a token
    /// worth cents in total is the population Jupiter has no route for, which
    /// comes back with no `usdPrice` at all and is dropped by
    /// `into_fetched_price` before this filter ever sees it.
    ///
    /// So the low end is guarded for the same reason as the high end below, and
    /// it is not token economics: `usd_price` is an unvalidated JSON number from
    /// a third party. `Decimal`'s maximum scale is 28, so its smallest positive
    /// value is `1e-28` — the whole band `[1e-28, 5e-19)` is representable in
    /// Rust, survives every `> 0` test, and rounds to a stored zero. That is the
    /// bound that matters here; the `7.9e28` cited below is the *other* end of
    /// the same property and does not bear on this one (a `Decimal` at scale 28
    /// tops out near `7.9228`, not `7.9e28` — the two bounds cannot hold at
    /// once). [`PriceProvider`] also reserves two more provenances, `Helius` and
    /// `Fallback`; neither has a `PriceSource` implementation today, which makes
    /// them a reason to keep the filter honest for a future writer rather than
    /// evidence of a second untrusted producer now. The guard is cheap; the
    /// input is not trusted.
    ///
    /// ## The high end — why a ceiling too
    ///
    /// [`PRICE_STORAGE_PRECISION`] leaves 20 integer digits, so a price that
    /// rounds to `10^20` or more is refused by the *column type itself* with
    /// `22003 numeric_field_overflow` — before any `CHECK` is consulted, so
    /// `009_price_positivity.sql` does not and cannot guard it. `Decimal` holds
    /// values up to ~7.9e28 and `usd_price` arrives from Jupiter unvalidated, so
    /// the value is reachable from the wire.
    ///
    /// It matters because the two failures have the *same* blast radius:
    /// `insert_batch` sends one statement, and `ON CONFLICT DO NOTHING` covers
    /// neither `23514` nor `22003`. Either one aborts the tick for every other
    /// mint. A filter that closed only the low end would leave the exact outage
    /// it was written to prevent reachable from the other side.
    ///
    /// ## Why this rounding mode
    ///
    /// `MidpointAwayFromZero` is Postgres's own `NUMERIC` rounding, verified
    /// against the server rather than assumed: `5e-19` stores as `1e-18`,
    /// `4e-19` stores as `0`. Using rust_decimal's default (banker's rounding)
    /// would disagree on the exact midpoint. Both bounds are compared *after*
    /// rounding, which is how Postgres words the overflow rule too.
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
