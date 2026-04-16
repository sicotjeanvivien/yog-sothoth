use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum RpcListenerError {
    #[error("WebSocket connection failed: {0}")]
    ConnectionFailed(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("all {count} subscription(s) failed")]
    AllSubscriptionsFailed { count: usize },

    #[error("no pools configured")]
    NoPoolsConfigured,

    #[error("PubSubClient error : {0}")]
    PubSubClient(String),
}
