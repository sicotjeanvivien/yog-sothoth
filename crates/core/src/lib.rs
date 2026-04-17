#[cfg(feature = "solana")]
pub mod amm;
pub mod domain;
pub mod error;
#[cfg(feature = "solana")]
pub mod protocols;

pub use error::CoreError;

/// Convenience result type for all yog-core operations.
pub type CoreResult<T> = Result<T, CoreError>;
