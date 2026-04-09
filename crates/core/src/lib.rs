pub mod amm;
pub mod error;
pub mod protocols;
pub mod domain;

pub use error::CoreError;

/// Convenience result type for all yog-core operations.
pub type CoreResult<T> = Result<T, CoreError>;
