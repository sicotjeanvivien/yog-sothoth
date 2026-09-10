//! Where the pipeline's work comes from — the port, and what travels through it.
//!
//! # Why this is a port and not a `match`
//!
//! The two acquisition models do not differ in their transport alone. The
//! JSON-RPC path **notifies and then asks**: a WebSocket pushes log lines, a
//! dispatcher filters them, and an HTTP call fetches each transaction back.
//! Yellowstone **delivers**: one stream carries whole transactions, with their
//! index in the block, and nothing is fetched. Half the chain — the fleet, the
//! dispatcher, the fetcher, the concurrency bound that the RPC quota dictates —
//! exists on one side and not the other.
//!
//! What is identical is everything *below* the transaction: extraction,
//! persistence, pool maintenance, the projections. So the seam belongs exactly
//! where the two chains converge, on a translated transaction, and that is what
//! this module names. A `match` in the daemon would have put it one storey too
//! high, with what is common written outside it and what differs inside — and
//! the two are not divided that way.
//!
//! An implementation therefore owns its whole acquisition sub-graph, however
//! many tasks that takes, and the daemon has one graph regardless of which one
//! runs:
//!
//! ```text
//! source → mpsc<IngestedTransaction> → IndexerWorker → TransactionProcessor
//! ```

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yog_core::{application::extraction::OnChainTransaction, domain::Protocol};

use crate::error::SourceError;

/// A transaction, translated and timestamped, with the protocol it belongs to.
///
/// # Why this is not a `QualifiedSignature`
///
/// Because the two paths hand over work at different points. `logsSubscribe`
/// notifies and the pipeline then *asks* — so what travels there is a signature,
/// and the fetch is a step of its own. Yellowstone **delivers**: the transaction
/// is already in hand, with its `index` in the block, and there is nothing left
/// to fetch.
///
/// Handing over here rather than at a signature is what lets the fetch belong to
/// the path that needs one. A `QualifiedSignature` still exists inside
/// `infra::rpc`, between its dispatcher and its fetcher, where it means
/// something; it just never leaves.
///
/// The protocol rides along rather than being re-derived. Both sources already
/// know it — the gRPC subscription's filters are named after protocols, and the
/// RPC path carries it from the subscription target — and re-deriving it
/// downstream would mean re-reading the instructions to answer a question that
/// was answered upstream.
#[derive(Debug)]
pub(crate) struct IngestedTransaction {
    pub(crate) protocol: Protocol,
    pub(crate) transaction: OnChainTransaction,
}

/// Delivers transactions to the pipeline, whatever it had to do to get them.
///
/// # What an implementation owes
///
/// - **Skip-and-log inside, propagate only loop-level failures.** A transaction
///   that will not translate is counted and stepped over; only a broken channel,
///   an exhausted retry budget or a closed semaphore comes back as
///   [`SourceError`]. This is the crate's rule and the port does not soften it.
/// - **Return `Ok(())` on a requested shutdown**, and on a downstream that has
///   gone away. Neither is a failure, and the daemon cancels the shared token on
///   any task's return regardless.
/// - **Honour back-pressure.** `downstream` is bounded, and a full channel means
///   the database is the bottleneck. Waiting is correct; dropping is not — but
///   the wait must stay interruptible by `shutdown`, or a full consumer makes
///   the process unstoppable.
#[async_trait]
pub(crate) trait TransactionSource: Send + Sync {
    /// Subscribe to one pool's transactions.
    ///
    /// Called before [`TransactionSource::run`], by
    /// [`WatchedPoolService::restore_subscriptions`], once per active row of
    /// `watched_pools`. It is on this trait rather than on a `PoolWatcher` of
    /// its own because what a source subscribes to and what it delivers are one
    /// subject — and because splitting it would buy a second trait for one
    /// method with one caller.
    ///
    /// [`WatchedPoolService::restore_subscriptions`]: crate::application::services::WatchedPoolService::restore_subscriptions
    async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey);

    /// Deliver transactions until shutdown, or until an unrecoverable failure.
    async fn run(
        &self,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), SourceError>;
}
