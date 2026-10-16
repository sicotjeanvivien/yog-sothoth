/// Failure of an external data source (HTTP, JSON-RPC, decoding), absorbed by the workers and
/// retried next tick; the conversion from `reqwest`, which strips the URL, is `infra/source_error.rs`.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Transport-level failure (DNS, connection, non-2xx).
    #[error("source HTTP error: {0}")]
    Http(String),

    /// HTTP 429, with the `Retry-After` delay when the response gave one.
    #[error("source rate-limited (429), retry_after={retry_after:?}")]
    RateLimited {
        retry_after: Option<std::time::Duration>,
    },

    /// A 2xx whose body could not be decoded into the expected shape.
    #[error("source decode error: {0}")]
    Decode(String),
}
