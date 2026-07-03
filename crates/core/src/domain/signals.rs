pub mod detector;
pub mod model;
pub mod repository;

pub use detector::{DetectorError, EvalContext, SignalDetector};
pub use model::{Severity, Signal, SignalRecord};
pub use repository::{SignalCursor, SignalFeed, SignalRepository};
