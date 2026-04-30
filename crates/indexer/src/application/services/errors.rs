//! Internal error types for the `services` module.
//!
//! These types stay private to `application::services` (`pub(super)` visibility):
//! the public boundary of `IndexerService::index_transaction` remains
//! `anyhow::Result<()>` so the worker does not need to know the internal
//! taxonomy of failures.
//!
//! The purpose of typing internal errors is to:
//!   - eliminate string-based heuristics like `err.to_string().contains("...")`
//!   - give each failure mode a stable metric label via `metric_label()`
//!   - make the caller's `match` over outcomes exhaustive and readable

use thiserror::Error;

/// Error returned by the RPC fetch layer of `IndexerService`.
///
/// `NotFound` is a distinct variant rather than `Ok(None)` so the
/// transaction-fetch signature stays unambiguous: every non-`Ok` outcome
/// is a failure mode, and the caller matches on the variant to decide
/// whether to treat it as a metric-only outcome (`NotFound`) or a real
/// error to propagate.
#[derive(Error, Debug)]
pub(super) enum FetchError {
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
    pub(super) fn metric_label(&self) -> &'static str {
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
    /// The retry loop in `fetch_transaction` already flattens the RPC
    /// client error into a `String` (closure constraint of `tokio_retry`).
    /// We classify it here once, at the boundary, so the rest of the
    /// service works with the typed variant.
    pub(super) fn from_rpc_string(msg: String) -> Self {
        let lower = msg.to_lowercase();
        if lower.contains("null") {
            // RPC returned a null result for the signature — treat as not found.
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
mod tests {
    use super::*;

    #[test]
    fn rate_limit_variants_are_classified() {
        for raw in [
            "HTTP 429 Too Many Requests",
            "rate limit exceeded",
            "Too Many Requests",
        ] {
            let e = FetchError::from_rpc_string(raw.to_string());
            assert!(matches!(e, FetchError::RateLimited), "raw = {raw:?}");
            assert_eq!(e.metric_label(), "rate_limited");
        }
    }

    #[test]
    fn timeout_is_classified() {
        let e = FetchError::from_rpc_string("request timed out".to_string());
        assert!(matches!(e, FetchError::Timeout));
        assert_eq!(e.metric_label(), "timeout");
    }

    #[test]
    fn null_response_maps_to_not_found() {
        let e = FetchError::from_rpc_string("got null in response".to_string());
        assert!(matches!(e, FetchError::NotFound));
        assert_eq!(e.metric_label(), "not_found");
    }

    #[test]
    fn unknown_falls_back_to_other() {
        let e = FetchError::from_rpc_string("some unexpected RPC error".to_string());
        assert!(matches!(e, FetchError::Other(_)));
        assert_eq!(e.metric_label(), "other");
    }

    #[test]
    fn connection_keyword_is_classified() {
        let e = FetchError::from_rpc_string("connection refused".to_string());
        assert!(matches!(e, FetchError::Connection(_)));
        assert_eq!(e.metric_label(), "connection_error");
    }
}
