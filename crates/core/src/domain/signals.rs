mod detector;
mod model;
mod repository;

pub use detector::{DetectorError, EvalContext, SignalDetector};
pub use model::{
    DETECTOR_FLOW_IMBALANCE, DETECTOR_PRICE_ORACLE_DEVIATION, DETECTOR_TVL_DRAIN, PairToken,
    Severity, Signal, SignalRecord, SignalValueUnit,
};
pub use repository::{SignalCursor, SignalFeed, SignalFilter, SignalRepository};
