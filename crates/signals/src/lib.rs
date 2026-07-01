//! `yog-signals` — the Signal Engine library.
//!
//! Turns observed data into [`Signal`](yog_core::domain::Signal)s. A
//! [`SignalEngine`] runs a set of detectors, each on its own cadence
//! (batch, per-detector), and persists what they emit through a
//! [`SignalRepository`](yog_core::domain::SignalRepository). The crate is
//! persistence-agnostic: it depends only on `yog-core` traits, and the
//! concrete `Pg` repositories are injected by the `signal-engine` binary.

pub mod detectors;
pub mod engine;
pub mod metrics;

pub use detectors::FlowImbalanceDetector;
pub use engine::{EngineError, SignalEngine};
pub use metrics::EngineMetrics;
