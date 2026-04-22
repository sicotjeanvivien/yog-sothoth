use thiserror::Error;

#[derive(Debug, Error)]
pub enum DispatcherError {
    #[error("dispatcher configured with no filters")]
    NoFilters,
}
