use thiserror::Error;

use crate::error::CredentialError;

#[derive(Debug, Error)]
pub(crate) enum RpcListenerError {
    /// The configured header could not be validated; the gRPC path refuses it the same way.
    #[error(transparent)]
    Credential(#[from] CredentialError),

    #[error("No subscription targets configured")]
    NoSubscriptionTargets,

    /// The endpoint is not a WebSocket, refused before any worker spawns: `INGEST_STREAM_URL` is
    /// shared by both sources.
    #[error("`INGEST_STREAM_URL` is not a usable WebSocket endpoint: {reason}")]
    InvalidEndpoint { reason: String },

    #[error("All Workers GaveUp failure: {failures}")]
    AllWorkersGaveUp { failures: String },
}
