use thiserror::Error;

/// What can stop the Yellowstone listener.
///
/// Only loop-level failures are here, per the crate's skip-and-log rule: a
/// transaction that will not translate is counted and stepped over inside the
/// loop and never reaches this enum.
///
/// ⚠️ **No variant carries a credential, and two of them are one word from
/// doing so.** The header variants name the *header*, never its value, and the
/// connection variants carry an error string that has been through
/// [`yog_bootstrap::SecretUrl::scrub`] — third-party transport errors quote the
/// URI they were handed, and that URI holds the key whenever the operator put
/// it there.
#[derive(Debug, Error)]
pub(crate) enum GrpcListenerError {
    #[error(
        "`INGEST_STREAM_HEADER_NAME` is not a valid header name: `{name}` — \
         it must be a token, e.g. `x-token`"
    )]
    InvalidHeaderName { name: String },

    /// The value is **not** quoted: it is the credential.
    #[error(
        "the value assembled for header `{name}` is not a valid header value — \
         check `INGEST_STREAM_HEADER_VALUE` and `INGEST_STREAM_KEY` (neither is \
         printed here)"
    )]
    InvalidHeaderValue { name: String },

    /// The same refusal `RpcListenerError::NoSubscriptionTargets` makes, for
    /// the same reason: a stream that subscribes to nothing opens, succeeds,
    /// and stays silent, which reads as a network problem and is a
    /// configuration one.
    #[error("no subscription targets configured")]
    NoSubscriptionTargets,

    #[error("`INGEST_STREAM_URL` is not a usable gRPC endpoint: {reason}")]
    InvalidEndpoint { reason: String },

    #[error("gRPC stream gave up after {attempts} connection attempts: {last_error}")]
    RetriesExhausted { attempts: u32, last_error: String },
}
