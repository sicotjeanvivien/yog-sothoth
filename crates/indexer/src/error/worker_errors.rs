#[derive(Debug, thiserror::Error)]
pub(crate) enum IndexerWorkerError {
    /// The concurrency semaphore was closed while the worker was still
    /// acquiring permits. Typically means a shutdown race — recoverable
    /// at the `Daemon` level.
    #[error("concurrency semaphore closed while acquiring permit")]
    SemaphoreClosed,
}
