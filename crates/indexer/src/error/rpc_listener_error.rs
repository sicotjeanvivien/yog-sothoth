use thiserror::Error;

use crate::error::CredentialError;

#[derive(Debug, Error)]
pub(crate) enum RpcListenerError {
    /// The configured header could not be validated — one rule, one type, and
    /// the same one the gRPC path raises.
    #[error(transparent)]
    Credential(#[from] CredentialError),

    #[error("No subscription targets configured")]
    NoSubscriptionTargets,

    /// The endpoint is not a WebSocket, refused before a single worker is
    /// spawned — the mirror of the gRPC path's own scheme refusal, and here for
    /// the same reason: `INGEST_STREAM_URL` is shared by the two sources, so
    /// switching one and forgetting the other is the ordinary mistake.
    #[error("`INGEST_STREAM_URL` is not a usable WebSocket endpoint: {reason}")]
    InvalidEndpoint { reason: String },

    #[error("All Workers GaveUp failure: {failures}")]
    AllWorkersGaveUp { failures: String },
}
