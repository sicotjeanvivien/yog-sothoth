//! The Solana account client's log lines, one function each.

use solana_pubkey::Pubkey;
use tracing::warn;

/// An account came back that could not be read, under `reason`.
pub(super) fn account_dropped(pool_address: &Pubkey, reason: &'static str) {
    warn!(
        pool_address = %pool_address,
        reason,
        "solana_account: account dropped as unreadable",
    );
}

/// The answer did not carry one entry per address asked; none of its accounts is kept.
pub(super) fn length_mismatch(asked: usize, answered: usize) {
    warn!(
        asked,
        answered, "solana_account: answer length differs from the addresses asked",
    );
}
