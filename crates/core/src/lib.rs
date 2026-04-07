pub mod amm;
pub mod error;
pub mod protocols;
pub mod types;

pub use error::CoreError;
pub use types::{LiquidityEvent, LiquidityEventType, PoolState, SwapEvent};

/// Convenience result type for all yog-core operations.
pub type CoreResult<T> = Result<T, CoreError>;
