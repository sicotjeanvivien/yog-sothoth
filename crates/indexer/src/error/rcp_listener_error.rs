use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum RpcListenerError {
    #[error("all {count} subscription(s) failed")]
    AllSubscriptionsFailed { count: usize },

    #[error("no protocols configured")]
    NoProtocolsConfigured,

    #[error("PubSubClient error : {0}")]
    PubSubClient(String),

    #[error("RPC WebSocket unreachable after {attempts} attempts: {message}")]
    MaxRetriesExceeded { attempts: u32, message: String },

    #[error("All Workers GaveUp failure: {failures}")]
    AllWorkersGaveUp { failures: String },
}
