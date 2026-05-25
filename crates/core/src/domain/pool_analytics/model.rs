//! Pool analytics — derived metrics over RPC-sourced data.
//!
//! Sits next to the other domain types but represents a distinct
//! class of data: nothing here comes from the chain directly.
//! [`PoolAnalytics`] is computed on demand by combining RPC-sourced
//! tables (`pool_current_state`, `swap_events`) with context tables
//! (`token_metadata`, `token_prices`).
//!
//! No analytics value is ever persisted into an RPC-sourced table.
//! When materialisation becomes necessary for performance, it will
//! land in a dedicated analytics table written by a separate job —
//! never by the indexer or by yog-context.

use rust_decimal::Decimal;

/// USD-denominated metrics for a single pool.
///
/// Both fields are `Option` because the inputs may not be fully
/// available:
///
///   - `tvl_usd` is `None` if the pool has no current state yet,
///     or if either token has no known price.
///   - `volume_24h_usd` is `None` if the pool has no swaps in the
///     last 24h whose tokens had a known price at the time of the
///     swap. A partial volume (some swaps priced, some not) is
///     returned as `Some(sum_of_priced_swaps)` — we surface what
///     we have rather than collapse the value because of partial
///     coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAnalytics {
    pub tvl_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
}

impl PoolAnalytics {
    /// Sentinel for "no analytics computable for this pool", used
    /// to fill the gaps when a requested pool address is missing
    /// from the repository batch result.
    pub fn empty() -> Self {
        Self {
            tvl_usd: None,
            volume_24h_usd: None,
        }
    }
}
