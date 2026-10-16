use thiserror::Error;

use crate::error::CredentialError;

/// What can stop the Yellowstone listener: loop-level failures only.
/// ⚠️ No variant carries a credential: header variants name the header, and connection errors go
/// through [`yog_bootstrap::SecretUrl::scrub`], which covers a key in the URI, not in a header.
#[derive(Debug, Error)]
pub(crate) enum GrpcListenerError {
    /// The configured header could not be validated; see [`CredentialError`].
    #[error(transparent)]
    Credential(#[from] CredentialError),

    /// Nothing to subscribe to: such a stream would open and stay silent, reading as a network fault.
    #[error("no subscription targets configured")]
    NoSubscriptionTargets,

    #[error("`INGEST_STREAM_URL` is not a usable gRPC endpoint: {reason}")]
    InvalidEndpoint { reason: String },

    #[error("gRPC stream gave up after {attempts} connection attempts: {last_error}")]
    RetriesExhausted { attempts: u32, last_error: String },
}
