//! HTTP fetcher for confirmed Solana transactions: a signature in, the parsed transaction or a
//! typed `FetchError` out, with the retry loop inside and no metric.

use super::transaction_adapter::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use std::sync::Arc;
use thiserror::Error;
use tokio_retry::{RetryIf, strategy::FixedInterval};
use yog_bootstrap::SecretUrl;

/// The highest transaction version `getTransaction` is told this crate reads; above, `-32015`, lost.
/// ⚠️ Not a setting: it moves only with a mainnet fixture of the version it names, which
/// `the_declared_ceiling_is_the_highest_fixture_version` holds equal; hence no environment variable.
pub(crate) const MAX_SUPPORTED_TRANSACTION_VERSION: u8 = 1;

/// Fetches confirmed transactions from a Solana RPC node with a bounded retry strategy.
pub(crate) struct TransactionFetcher {
    rpc_client: Arc<RpcClient>,
    /// Kept only to scrub the URL, credential included, out of the error strings reqwest renders.
    rpc_url: SecretUrl,
}

impl TransactionFetcher {
    pub(crate) fn new(rpc_client: Arc<RpcClient>, rpc_url: SecretUrl) -> Self {
        Self {
            rpc_client,
            rpc_url,
        }
    }

    /// Fetch a confirmed transaction by signature, retrying up to 5 times at 500 ms intervals
    /// what [`FetchError::is_retryable`] allows.
    /// ⚠️ `JsonParsed` is what `transaction_adapter` reads, not a preference.
    pub(crate) async fn fetch(
        &self,
        signature: Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta, FetchError> {
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
        };

        let strategy = FixedInterval::from_millis(500).take(5);
        RetryIf::start(
            strategy,
            || async {
                self.rpc_client
                    .get_transaction_with_config(&signature, config)
                    .await
                    // Scrubbed where the third party's string is born, not at each log site.
                    .map_err(|e| FetchError::from_rpc_string(self.rpc_url.scrub(&e.to_string())))
            },
            FetchError::is_retryable,
        )
        .await
    }
}

/// Error returned by the RPC fetch layer; `NotFound` is a variant, not `Ok(None)`, so every
/// non-`Ok` outcome is a failure mode the caller matches on.
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

    /// The RPC refused the transaction's version (`-32015`), above
    /// [`MAX_SUPPORTED_TRANSACTION_VERSION`]: deterministic, and a loss.
    #[error("transaction version refused: {0}")]
    UnsupportedVersion(String),

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
            FetchError::UnsupportedVersion(_) => "unsupported_version",
            FetchError::Other(_) => "other",
        }
    }

    /// Whether the fetch loop should ask again: everything but a version refusal, which is
    /// deterministic. `NotFound` stays retryable, the node may not have indexed it yet.
    pub(crate) fn is_retryable(&self) -> bool {
        !matches!(self, FetchError::UnsupportedVersion(_))
    }

    /// Classify a raw RPC error string into a typed variant, once, at the boundary.
    /// ⚠️ The version refusal is matched on its code, `-32015`, not its wording: the code is the
    /// protocol's, the sentence the provider's.
    fn from_rpc_string(msg: String) -> Self {
        let lower = msg.to_lowercase();
        if lower.contains("-32015") {
            FetchError::UnsupportedVersion(msg)
        } else if lower.contains("null") {
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
#[path = "tests/transaction_fetcher_tests.rs"]
mod tests;
