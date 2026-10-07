//! The price worker's log lines, one function each. A tick's ending logs in
//! `tick_outcome`, beside its label.

use tracing::{debug, info, warn};
use yog_core::domain::{KeptPrices, TokenPrice, UnpricedMints};

/// The worker starts, stating the two bounds its cadence entails, which an
/// operator cannot read off the configuration.
pub(super) fn started(cadence: std::time::Duration, kept: &KeptPrices, unpriced: &UnpricedMints) {
    info!(
        cadence_secs = cadence.as_secs(),
        rewrite_at_most_every_secs = kept.rewrites_at_most_every().num_seconds(),
        unpriced_asked_again_at_most_every_secs = unpriced.asks_again_at_most_every().num_seconds(),
        "PriceWorker started — a motionless price is rewritten at the floor, \
         a mint without a price is asked again at the cap"
    );
}

/// A stop was asked; the worker leaves between two ticks.
pub(super) fn stopping() {
    info!("shutdown requested — price worker stopping");
}

/// A tick asks the source for `count` of the `known` mints.
pub(super) fn pricing(count: usize, known: usize) {
    debug!(count, known, "price worker: pricing mints");
}

/// Prices the column cannot hold were dropped before the insert.
pub(super) fn unstorable(rejected: &[TokenPrice]) {
    warn!(
        count = rejected.len(),
        mints = ?rejected.iter().map(|p| p.mint.to_string()).collect::<Vec<_>>(),
        "price worker: dropped prices the price column cannot hold"
    );
}
