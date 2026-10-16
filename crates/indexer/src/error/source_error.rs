use thiserror::Error;

use crate::error::{GrpcListenerError, RpcListenerError};

/// What can stop a [`TransactionSource`]: loop-level failures only, each source mapping its own
/// typed errors here at the boundary.
///
/// [`TransactionSource`]: crate::application::source::TransactionSource
#[derive(Debug, Error)]
pub(crate) enum SourceError {
    /// The JSON-RPC fleet gave up, or could not be built.
    #[error(transparent)]
    RpcListener(#[from] RpcListenerError),

    /// The Yellowstone stream gave up, or its configuration cannot produce a
    /// subscription.
    #[error(transparent)]
    GrpcListener(#[from] GrpcListenerError),

    /// A bounded-concurrency stage lost its semaphore while acquiring a permit.
    #[error("{stage}: concurrency semaphore closed while acquiring permit")]
    SemaphoreClosed { stage: &'static str },

    /// A task a source spawned panicked: a source whose fetch stage is gone would look alive while
    /// delivering nothing.
    #[error("{task} panicked: {reason}")]
    TaskPanicked { task: &'static str, reason: String },
}
