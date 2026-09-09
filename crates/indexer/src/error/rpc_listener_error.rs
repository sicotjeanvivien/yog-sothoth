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

    #[error("All Workers GaveUp failure: {failures}")]
    AllWorkersGaveUp { failures: String },
}
