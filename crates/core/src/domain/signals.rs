mod detector;
mod model;
mod repository;

pub use detector::{DetectorError, EvalContext, SignalDetector};
pub use model::{Severity, Signal, SignalRecord};
pub use repository::{SignalCursor, SignalFeed, SignalRepository};
