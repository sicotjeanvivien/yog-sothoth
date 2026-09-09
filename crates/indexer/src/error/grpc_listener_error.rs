use thiserror::Error;

use crate::error::CredentialError;

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
///
/// ⚠️ **And `scrub` covers one carrier of the two.** A credential riding in a
/// metadata header has no equivalent, which `yog_bootstrap::Endpoint`'s module
/// docs state as a known gap rather than an oversight: it would mean guessing
/// at the shape of an error no client here has been seen to produce, and
/// guessing at shapes is what the redactor this workspace deleted did. What is
/// done instead is upstream — the value is marked sensitive, so the layers that
/// dump request metadata skip it. Written here because this is where somebody
/// would look for it.
#[derive(Debug, Error)]
pub(crate) enum GrpcListenerError {
    /// The configured header could not be validated — one rule, one type, see
    /// [`CredentialError`].
    #[error(transparent)]
    Credential(#[from] CredentialError),

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
