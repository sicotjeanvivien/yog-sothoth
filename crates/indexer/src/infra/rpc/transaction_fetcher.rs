//! HTTP fetcher for confirmed Solana transactions.
//!
//! Single responsibility: given a signature, return the parsed transaction
//! or a typed `FetchError`. The retry loop is contained here; metric
//! instrumentation is the caller's responsibility — no domain awareness
//! inside the fetcher.

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

/// The highest transaction version `getTransaction` is told this crate reads.
///
/// ⚠️ **A statement about the adapter, not a setting.** The RPC refuses any
/// transaction above it with `-32015`, and a refused transaction is lost for
/// good — nothing asks for its signature again. Raising it is what fixes that;
/// raising it past what `transaction_adapter` has been shown to read trades a
/// counted refusal for a reading that may be silently wrong. So it moves only
/// with a mainnet fixture of the version it names, and
/// `the_declared_ceiling_is_the_highest_fixture_version` holds the two equal.
///
/// Not an environment variable for the same reason: an operator could raise it
/// without the fixture that justifies it.
///
/// Version 1 is the format Solana activated on mainnet on 15 September 2026
/// (epoch 1035): up to 4 096 bytes, no address lookup tables, the compute
/// budget in a `transactionConfig` the adapter does not read. Opting in is
/// optional for a sender and mandatory for a reader.
pub(crate) const MAX_SUPPORTED_TRANSACTION_VERSION: u8 = 1;

/// Fetches confirmed transactions from a Solana RPC node with a bounded
/// retry strategy.
pub(crate) struct TransactionFetcher {
    rpc_client: Arc<RpcClient>,
    /// The endpoint the client talks to, kept only to scrub it back out of the
    /// error strings `solana-client` builds — reqwest renders the whole URL,
    /// credential included, and `FetchError` carries that string onward.
    rpc_url: SecretUrl,
}

impl TransactionFetcher {
    pub(crate) fn new(rpc_client: Arc<RpcClient>, rpc_url: SecretUrl) -> Self {
        Self {
            rpc_client,
            rpc_url,
        }
    }

    /// Fetch a confirmed transaction by signature.
    ///
    /// Retries up to 5 times at 500ms intervals, but only what
    /// [`FetchError::is_retryable`] allows: each failure is classified into a
    /// typed `FetchError` inside the loop, so the decision to try again is
    /// made on the variant rather than on a string.
    ///
    /// The `JsonParsed` encoding is not a preference: it is what
    /// `transaction_adapter` reads (the `PartiallyDecoded` inner instructions
    /// only that encoding produces). The two are siblings in this module tree
    /// so they cannot drift apart.
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
                    // Scrub *here*, where the third party's string is born,
                    // rather than at each site that later logs it: `yog-context`
                    // learned that a rule every present and future log site must
                    // remember is a rule that gets forgotten — nine sites, 38
                    // leaked lines, 2 September 2026.
                    .map_err(|e| FetchError::from_rpc_string(self.rpc_url.scrub(&e.to_string())))
            },
            FetchError::is_retryable,
        )
        .await
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

    /// The RPC refused the transaction's version (`-32015`): it is above
    /// [`MAX_SUPPORTED_TRANSACTION_VERSION`]. Deterministic — asking again gets
    /// the same answer — and a loss, since the signature is not asked for
    /// again: the day the chain moves past the ceiling, this is the variant
    /// that says so.
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

    /// Whether the fetch loop should ask again after this failure.
    ///
    /// Everything is retried except a version refusal, which is deterministic:
    /// the sixth attempt fails exactly like the first. `NotFound` stays
    /// retryable — a transaction the node has not indexed yet can appear a
    /// moment later.
    pub(crate) fn is_retryable(&self) -> bool {
        !matches!(self, FetchError::UnsupportedVersion(_))
    }

    /// Classify a raw RPC error string into a typed variant.
    ///
    /// `solana-client` hands the failure over as an error whose only stable
    /// content is its rendered text, so the classification reads that text —
    /// once, at the boundary, so the rest of the service works with the typed
    /// variant.
    ///
    /// The version refusal is tested first and on its JSON-RPC code, `-32015`,
    /// rather than on its wording: the code is the protocol's, the sentence is
    /// the provider's.
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
