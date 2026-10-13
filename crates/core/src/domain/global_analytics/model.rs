//! Protocol-wide aggregate analytics — derived metrics over *every*
//! observed pool at once.
//!
//! Distinct aggregate from [`crate::domain::PoolAnalytics`], which is scoped
//! to a single pool (identity = pool address). [`GlobalAnalytics`] has no
//! per-pool identity: it is a singleton-shaped roll-up of the whole observed
//! universe, the analytics counterpart of [`crate::domain::NetworkStatus`].
//! The two share only the trade-time USD valuation *logic*, not identity or
//! lifecycle — hence a separate module.

use rust_decimal::Decimal;

/// Protocol-wide aggregate analytics over every observed pool.
///
/// Powers `GET /api/stats`. The USD fields follow the same partial-coverage
/// rule as [`crate::domain::PoolAnalytics`]: they sum what is priceable and
/// surface that, rather than collapsing to `None` because some pools lack a
/// price.
///
///   - `total_tvl_usd` is the TVL yog-analytic last measured, summed over the
///     pools priced then. `None` when none was, or when that measurement is
///     older than the previous hour.
///   - `pools_priced` is how many pools contributed to `total_tvl_usd` — the
///     coverage numerator. The denominator (total observed) lives on the pool
///     inventory counts; the presentation layer derives the "N / M priced"
///     coverage from the two.
///   - `volume_24h_usd` / `fees_24h_usd` are the summed realized volume and
///     trading fee over the last 24h, valued at trade-time prices.
///   - `swap_buckets_24h` / `swap_buckets_priced_24h` are the coverage of those
///     two, in the same numerator/denominator spirit as `pools_priced`: how
///     many (pool, hour) buckets had swaps, and how many of them the valuation
///     covered. Without them a partially covered sum reads exactly like a
///     complete one — see [`crate::domain::PoolAnalytics`] for the full
///     rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAnalytics {
    pub total_tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
    pub swap_buckets_24h: i64,
    pub swap_buckets_priced_24h: i64,
}
