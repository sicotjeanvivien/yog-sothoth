//! The Yellowstone acquisition model, behind the port.
//!
//! Thin, and the thinness is the point: where [`super::super::rpc::source`]
//! assembles three stages and two channels because `logsSubscribe` notifies and
//! the pipeline then asks, this one has a single connection that **delivers**.
//! `GrpcListener::run` already produces the port's own
//! [`IngestedTransaction`] — the translation, the slot/time pairing and the
//! reconnection all live inside it — so there is nothing here but the
//! delegation.
//!
//! # What the port asks, and why this side already answered
//!
//! [`TransactionSource`] states three obligations, and the JSON-RPC source had
//! to learn all three in review. This one was built with them, in the slice
//! that wrote the listener:
//!
//! - **skip-and-log inside** — `session` counts every dropped update by reason
//!   and steps over it; only a broken stream reaches [`GrpcListenerError`];
//! - **`Ok(())` on a shutdown and on a departed consumer** — `Attempt::
//!   ShutdownRequested` and `Attempt::DownstreamClosed`, both already exits of
//!   `GrpcListener::run`;
//! - **an interruptible wait** — `SessionState::ShutdownRequested` is what makes
//!   a full consumer stoppable, and it exists because a review found the
//!   opposite first.
//!
//! [`GrpcListenerError`]: crate::error::GrpcListenerError

use std::sync::Arc;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yog_core::domain::Protocol;

use crate::{
    application::source::{IngestedTransaction, TransactionSource},
    error::SourceError,
    infra::grpc::GrpcListener,
};

/// The delivering source: one Yellowstone stream, no fetch.
pub(crate) struct GrpcTransactionSource {
    listener: Arc<GrpcListener>,
}

impl GrpcTransactionSource {
    pub(crate) fn new(listener: Arc<GrpcListener>) -> Self {
        Self { listener }
    }
}

#[async_trait]
impl TransactionSource for GrpcTransactionSource {
    async fn watch_protocol(&self, protocol: Protocol) {
        self.listener.watch(protocol).await;
    }

    async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.listener.watch_pool(protocol, pool_address).await;
    }

    async fn run(
        &self,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), SourceError> {
        Arc::clone(&self.listener)
            .run(downstream, shutdown)
            .await
            .map_err(SourceError::from)
    }
}
