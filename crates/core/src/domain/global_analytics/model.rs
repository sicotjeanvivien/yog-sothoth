//! Protocol-wide aggregate analytics — derived metrics over *every*
//! observed pool at once.
//!
//! Distinct aggregate from [`crate::domain::PoolAnalytics`], which is scoped
//! to a single pool (identity = pool address). [`GlobalAnalytics`] has no
//! per-pool identity: it is a singleton-shaped roll-up of the whole observed
//! universe, the analytics counterpart of [`crate::domain::NetworkStatus`].
//! The two share only the trade-time USD valuation *logic*, not identity or
//! lifecycle — hence a separate module.
//!
//! Nothing here is persisted: like `PoolAnalytics`, the values are computed on
//! demand by combining RPC-sourced tables with the context price tables. When
//! materialisation becomes necessary it will move to a dedicated analytics
//! store, never written by the indexer or by yog-context.

use rust_decimal::Decimal;

/// Protocol-wide aggregate analytics over every observed pool.
///
/// Powers `GET /api/stats`. The USD fields follow the same partial-coverage
/// rule as [`crate::domain::PoolAnalytics`]: they sum what is priceable and
/// surface that, rather than collapsing to `None` because some pools lack a
/// price.
///
///   - `total_tvl_usd` is the summed current TVL across all pools that have a
///     current state and a known price for both tokens. `None` only when not a
///     single pool is priceable.
///   - `pools_priced` is how many pools contributed to `total_tvl_usd` — the
///     coverage numerator. The denominator (total observed) lives on the pool
///     inventory counts; the presentation layer derives the "N / M priced"
///     coverage from the two.
///   - `volume_24h_usd` / `fees_24h_usd` are the summed realized volume and
///     trading fee over the last 24h, valued at trade-time prices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAnalytics {
    pub total_tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
}
