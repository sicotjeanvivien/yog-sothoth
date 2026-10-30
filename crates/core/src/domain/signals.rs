mod detector;
mod model;
mod repository;

pub use detector::{DetectorError, EvalContext, SignalDetector};
pub use model::{
    FLOW_IMBALANCE, PRICE_ORACLE_DEVIATION, PairToken, Severity, Signal, SignalRecord,
    SignalValueUnit, TVL_DRAIN,
};
pub use repository::{SignalCursor, SignalFeed, SignalFilter, SignalRepository};
