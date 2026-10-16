//! Where the pipeline's work comes from: the port, placed where the two acquisition models
//! converge. JSON-RPC notifies then asks, Yellowstone delivers; below a translated transaction
//! extraction, persistence and maintenance are one pipeline.

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yog_core::{application::extraction::OnChainTransaction, domain::Protocol};

use crate::error::SourceError;

/// A transaction, translated and timestamped, with the protocol it belongs to.
/// Handed over after any fetch, so the fetch belongs to the path that needs one; the protocol
/// rides along because both sources already know it.
#[derive(Debug)]
pub(crate) struct IngestedTransaction {
    pub(crate) protocol: Protocol,
    pub(crate) transaction: OnChainTransaction,
}

/// Delivers transactions to the pipeline: skips and logs per transaction, returns `Ok(())` on
/// shutdown or a departed consumer, waits on a full `downstream` rather than drop.
/// ⚠️ That wait stays interruptible by `shutdown`, or a full consumer makes the process unstoppable.
#[async_trait]
pub(crate) trait TransactionSource: Send + Sync {
    /// Subscribe to a whole protocol, before `run`, under `INGEST_SCOPE=protocols` only.
    /// ⚠️ Only what `ExtractionDispatcher::implemented_protocols` names: a stub's program id would
    /// buy a firehose to discard.
    async fn watch_protocol(&self, protocol: Protocol);

    /// Subscribe to one pool, before `run`, once per active `watched_pools` row
    /// ([`WatchedPoolService::restore_subscriptions`]); under `INGEST_SCOPE=pools` only.
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
