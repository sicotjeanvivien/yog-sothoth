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

    /// HTTP 429 — the provider is rate-limiting us. Carries the
    /// `Retry-After` delay when the response provided one, so the
    /// caller can pace its retry instead of guessing.
    #[error("source rate-limited (429), retry_after={retry_after:?}")]
    RateLimited {
        retry_after: Option<std::time::Duration>,
    },

    /// The response was 2xx but the body could not be decoded into
    /// the expected shape.
    #[error("source decode error: {0}")]
    Decode(String),
}

/// Convert a `reqwest` failure, **stripping the URL first**.
///
/// This exists so the secret cannot enter the error type at all. reqwest's own
/// documentation warns about it — *"Errors may include the full URL used to
/// make the Request. If the URL contains sensitive information (e.g. an API key
/// as a query parameter), be sure to remove it using `without_url()`"* — and on
/// 2 September 2026 this crate wrote the Helius API key into **38 log lines**
/// because nine call sites each recopied `e.to_string()` unredacted.
///
/// The redaction is here, at the boundary, rather than at the logging call
/// sites, and that placement is the whole point: a rule that every present and
/// future log site must remember is a rule that will be forgotten. Nothing that
/// comes out of here carries a URL, so no call site can leak one.
///
/// `yog-indexer` used to make the other choice — `utils::redact_api_key`,
/// applied when formatting. It was removed in September 2026, for a second
/// reason on top of the placement: matching the literal `api-key=` made it
/// blind to a credential in a URL *path*, which is how Alchemy and QuickNode
/// authenticate. It now scrubs at the boundary too, through
/// `SecretUrl::scrub`, which knows its own secret instead of a pattern.
///
/// Losing the URL costs little: which endpoint was called is in the
/// configuration, and the provider is already named by the log's target.
impl From<reqwest::Error> for SourceError {
    fn from(e: reqwest::Error) -> Self {
        // Read the kind *before* `without_url`, which consumes the error.
        // The classification reproduces what the nine call sites did by hand:
        // `.json::<T>()` failures are decode errors, everything else — connect,
        // timeout, non-2xx via `error_for_status` — is transport.
        let is_decode = e.is_decode();
        let redacted = e.without_url();

        // reqwest's own `Display` renders the kind and the URL, and *not* the
        // cause. Strip the URL and a connect failure, a DNS failure, a TLS
        // handshake failure and both timeouts all render the same four words —
        // the URL was accidentally carrying the only distinguishing detail.
        // The cause chain carries none of the secret (reqwest attaches the URL
        // at the top level only, which `without_url` has just removed), so it
        // is appended: that is what keeps the log readable after redaction.
        let mut message = redacted.to_string();
        let mut cause = std::error::Error::source(&redacted);
        while let Some(current) = cause {
            message.push_str(": ");
            message.push_str(&current.to_string());
            cause = current.source();
        }

        if is_decode {
            Self::Decode(message)
        } else {
            Self::Http(message)
        }
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
