use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DispatcherError {
    #[error("dispatcher configured with no filters")]
    NoFilters,
}
