//! HTTP fetcher for confirmed Solana transactions.
//!
//! Single responsibility: given a signature, return the parsed transaction
//! or a typed `FetchError`. The retry loop is contained here; metric
//! instrumentation is the caller's responsibility — no domain awareness
//! inside the fetcher.

use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use std::sync::Arc;
use thiserror::Error;
use tokio_retry::{Retry, strategy::FixedInterval};
use yog_core::solana_types::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};

/// Fetches confirmed transactions from a Solana RPC node with a bounded
/// retry strategy.
pub(crate) struct TransactionFetcher {
    rpc_client: Arc<RpcClient>,
}

impl TransactionFetcher {
    pub(crate) fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    /// Fetch a confirmed transaction by signature.
    ///
    /// Retries up to 5 times at 500ms intervals. The raw RPC error string
    /// is classified into a typed `FetchError` at the boundary.
    pub(crate) async fn fetch(
        &self,
        signature: Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta, FetchError> {
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let strategy = FixedInterval::from_millis(500).take(5);
        Retry::start(strategy, || async {
            self.rpc_client
                .get_transaction_with_config(&signature, config)
                .await
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(FetchError::from_rpc_string)
    }
}

/// Error returned by the RPC fetch layer.
///
/// `NotFound` is a distinct variant rather than `Ok(None)` so the
/// transaction-fetch signature stays unambiguous: every non-`Ok` outcome
/// is a failure mode, and the caller matches on the variant to decide
/// whether to treat it as a metric-only outcome (`NotFound`) or a real
/// error to propagate.
#[derive(Error, Debug)]
pub(crate) enum FetchError {
    #[error("transaction not found after retries")]
    NotFound,

    #[error("RPC rate limit hit")]
    RateLimited,

    #[error("RPC request timed out")]
    Timeout,

    #[error("connection error: {0}")]
    Connection(String),

    #[error("RPC error: {0}")]
    Other(String),
}

impl FetchError {
    /// Stable label used as a metric tag — must remain low-cardinality.
    pub(crate) fn metric_label(&self) -> &'static str {
        match self {
            FetchError::NotFound => "not_found",
            FetchError::RateLimited => "rate_limited",
            FetchError::Timeout => "timeout",
            FetchError::Connection(_) => "connection_error",
            FetchError::Other(_) => "other",
        }
    }

    /// Classify a raw RPC error string into a typed variant.
    ///
    /// The retry loop already flattens the RPC client error into a
    /// `String` (closure constraint of `tokio_retry`). We classify it
    /// here once, at the boundary, so the rest of the service works
    /// with the typed variant.
    fn from_rpc_string(msg: String) -> Self {
        let lower = msg.to_lowercase();
        if lower.contains("null") {
            FetchError::NotFound
        } else if lower.contains("429")
            || lower.contains("rate limit")
            || lower.contains("too many requests")
        {
            FetchError::RateLimited
        } else if lower.contains("timeout") || lower.contains("timed out") {
            FetchError::Timeout
        } else if lower.contains("connection") || lower.contains("connect") {
            FetchError::Connection(msg)
        } else {
            FetchError::Other(msg)
        }
    }
}

#[cfg(test)]
#[path = "transaction_fetcher_tests.rs"]
mod tests;
