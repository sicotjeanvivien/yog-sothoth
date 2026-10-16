#[derive(Debug, thiserror::Error)]
pub(crate) enum IndexerWorkerError {
    /// The concurrency semaphore closed while the worker was acquiring a permit.
    #[error("concurrency semaphore closed while acquiring permit")]
    SemaphoreClosed,
}
