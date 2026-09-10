use thiserror::Error;

use crate::error::{DispatcherError, GrpcListenerError, RpcListenerError};

/// What can stop a [`TransactionSource`].
///
/// The port's error type, and therefore the one place where the two
/// acquisition models are the same shape. Each source keeps its own typed
/// errors internally — the fleet's, the dispatcher's, the stream's — and maps
/// them here at the boundary, per the crate's rule that a `?` crossing a
/// boundary maps explicitly.
///
/// ⚠️ **Only loop-level failures reach this enum**, on both paths. A
/// transaction that cannot be fetched, translated or persisted is counted and
/// stepped over inside the source; what comes out here is a source that can no
/// longer deliver at all.
///
/// [`TransactionSource`]: crate::application::source::TransactionSource
#[derive(Debug, Error)]
pub(crate) enum SourceError {
    /// The JSON-RPC fleet gave up, or could not be built.
    #[error(transparent)]
    RpcListener(#[from] RpcListenerError),

    /// The filter chain the JSON-RPC path runs between its listener and its
    /// fetcher — a configuration failure, raised before anything flows.
    #[error(transparent)]
    Dispatcher(#[from] DispatcherError),

    /// The Yellowstone stream gave up, or its configuration cannot produce a
    /// subscription.
    #[error(transparent)]
    GrpcListener(#[from] GrpcListenerError),

    /// A bounded-concurrency stage lost its semaphore while acquiring a permit.
    /// A shutdown race in practice; recoverable at the `Daemon` level, which
    /// cancels everything anyway.
    #[error("{stage}: concurrency semaphore closed while acquiring permit")]
    SemaphoreClosed { stage: &'static str },

    /// One of the tasks a source spawned did not return — it panicked. Named
    /// rather than swallowed: a source whose fetch stage is gone still holds an
    /// open subscription, and would look alive while delivering nothing.
    #[error("{task} panicked: {reason}")]
    TaskPanicked { task: &'static str, reason: String },
}
