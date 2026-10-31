//! The price worker's log lines, one function each. A tick's ending logs in
//! `tick_outcome`, beside its label.

use tracing::{debug, info, warn};
use yog_core::domain::{KeptPrices, TokenPrice, UnpricedMints};

/// The worker starts, stating what its cadence entails for a mint at its turn.
pub(super) fn started(cadence: std::time::Duration, kept: &KeptPrices, unpriced: &UnpricedMints) {
    info!(
        cadence_secs = cadence.as_secs(),
        rewrite_after_secs = kept.rewrites_at_most_every().num_seconds(),
        unpriced_wait_cap_secs = unpriced.asks_again_at_most_every().num_seconds(),
        "PriceWorker started — at its turn, a motionless price is rewritten past the floor, \
         a mint without a price is asked again past the cap"
    );
}

/// A stop was asked; the worker leaves between two ticks.
pub(super) fn stopping() {
    info!("shutdown requested — price worker stopping");
}

/// A tick asks the source for `count` of the `candidates`.
pub(super) fn pricing(count: usize, candidates: usize) {
    debug!(count, candidates, "price worker: pricing mints");
}

/// Prices the column cannot hold were dropped before the insert.
pub(super) fn unstorable(rejected: &[TokenPrice]) {
    warn!(
        count = rejected.len(),
        mints = ?rejected.iter().map(|p| p.mint.to_string()).collect::<Vec<_>>(),
        "price worker: dropped prices the price column cannot hold"
    );
}
