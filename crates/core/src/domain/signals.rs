mod detector;
mod detector_error;
mod model;
mod read;
mod repository;

pub use detector::{EvalContext, SignalDetector};
pub use detector_error::DetectorError;
pub use model::{
    DETECTOR_FLOW_IMBALANCE, DETECTOR_TVL_DRAIN, PairToken, Severity, Signal, SignalRecord,
    SignalValueUnit,
};
pub use read::{SignalCursor, SignalFilter};
pub use repository::{SignalFeed, SignalRepository};
