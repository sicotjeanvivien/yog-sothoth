//! The metadata worker's log lines, one function each. A tick's ending logs in
//! `tick_outcome`, beside its label.

use solana_pubkey::Pubkey;
use tracing::{debug, info, warn};
use yog_core::RepositoryError;

use super::scope::Scope;

pub(super) fn started() {
    info!("MetadataWorker started");
}

/// A stop was asked; the worker leaves between two ticks.
pub(super) fn stopping() {
    info!("shutdown requested — metadata worker stopping");
}

/// A tick asks the source for `count` mints.
pub(super) fn enriching(count: usize, scope: Scope) {
    debug!(
        count,
        scope = scope.as_label(),
        "metadata worker: enriching mints"
    );
}

pub(super) fn upserted(mint: &Pubkey) {
    debug!(mint = %mint, "metadata worker: upserted");
}

pub(super) fn upsert_failed(mint: &Pubkey, error: &RepositoryError) {
    warn!(
        mint = %mint,
        error = %error,
        "metadata worker: upsert failed",
    );
}
