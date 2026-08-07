mod flow_imbalance;
mod price_oracle_deviation;
mod tvl_drain;

/// Shared harness for the tests that assert a skip was *counted*, not merely
/// not-signalled.
#[cfg(test)]
mod metrics_probe;

pub use flow_imbalance::{FlowImbalanceDetector, FlowImbalanceSettings};
pub use price_oracle_deviation::{PriceOracleDeviationDetector, PriceOracleDeviationSettings};
pub use tvl_drain::{TvlDrainDetector, TvlDrainSettings};
