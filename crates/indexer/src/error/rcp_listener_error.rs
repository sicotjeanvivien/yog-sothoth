use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum RpcListenerError {
    #[error("No subscription targets configured")]
    NoSubscriptionTargets,

    #[error("All Workers GaveUp failure: {failures}")]
    AllWorkersGaveUp { failures: String },
}
