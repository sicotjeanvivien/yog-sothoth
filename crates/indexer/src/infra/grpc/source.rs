//! The Yellowstone acquisition model behind the port: one connection that delivers, so nothing
//! here but delegation to `GrpcListener::run`, which translates, pairs slot and time, and reconnects.

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
