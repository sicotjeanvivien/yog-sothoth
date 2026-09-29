//! The signal engine: one poll loop per detector, deduplication, persistence.

mod metrics;
mod signal_engine;

pub(crate) use metrics::EngineMetrics;
pub(crate) use signal_engine::SignalEngine;
