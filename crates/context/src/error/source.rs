// ── To ADD to crates/context/src/error.rs ────────────────────────────
//
// Alongside ConfigError, BootstrapError, WorkerError.

/// Failure of an external data source (HTTP, JSON-RPC, decoding).
///
/// Returned by the source clients (`HeliusDasClient`,
/// `JupiterPriceClient`). The workers absorb these errors in their
/// loop (log + retry on the next tick) rather than propagating —
/// `yog-context` is resilient by design: an external hiccup must not
/// take the daemon down.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Transport-level failure (DNS, connection, non-2xx).
    #[error("source HTTP error: {0}")]
    Http(String),

    /// The response was 2xx but the body could not be decoded into
    /// the expected shape.
    #[error("source decode error: {0}")]
    Decode(String),
}
