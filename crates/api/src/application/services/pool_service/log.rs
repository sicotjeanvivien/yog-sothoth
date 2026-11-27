//! The pool service's log lines, one function each.

use solana_pubkey::Pubkey;
use tracing::warn;
use yog_core::domain::FeeSchedulerDrop;

/// A stored fee curve could not be rebuilt: the pool is served without it.
pub(super) fn fee_scheduler_dropped(pool: &Pubkey, drop: FeeSchedulerDrop) {
    match drop {
        FeeSchedulerDrop::OutOfRange(out) => warn!(
            pool = %pool,
            field = out.field,
            "stored fee curve out of range — pool served without it",
        ),
        FeeSchedulerDrop::PartlyNull => warn!(
            pool = %pool,
            "stored fee curve partly NULL — pool served without it",
        ),
    }
}
