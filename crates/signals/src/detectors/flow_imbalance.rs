//! Flow imbalance detector.
//!
//! Measures directional pressure on a pool: over a trailing window, how
//! lopsided is swap volume between the two trade directions? It reads
//! per-pool directional USD volume ([`SwapFlowRepository`]) and emits a
//! signal when the normalised imbalance
//!
//! ```text
//!   imbalance = (a_to_b_usd - b_to_a_usd) / (a_to_b_usd + b_to_a_usd)  ∈ [-1, +1]
//! ```
//!
//! crosses a threshold — provided total volume clears a floor, without which
//! a near-dead pool with two dust swaps would read a spurious ±1.
//!
//! Stateless between ticks: it recomputes from the DB snapshot each time.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use yog_core::domain::{
    DetectorError, EvalContext, Protocol, Severity, Signal, SignalDetector, SwapFlowRepository,
};

/// Tuning knobs of the flow-imbalance detector, as loaded from the
/// environment by the bootstrap config. A named-field struct rather than
/// constructor arguments: half of these share a type (`Decimal` ×3,
/// durations ×3), so positional passing would let a swapped pair compile
/// silently.
pub struct FlowImbalanceSettings {
    /// Trailing window over which volume is aggregated.
    pub window: ChronoDuration,
    /// How often the engine ticks this detector.
    pub interval: Duration,
    /// Rolling per-pool suppression window (engine-level dedup).
    pub cooldown: Duration,
    /// Minimum total USD volume in the window for a pool to be considered.
    pub min_volume_usd: Decimal,
    /// `|imbalance|` at or above which a signal is emitted.
    pub threshold: Decimal,
    /// `|imbalance|` at or above which the signal escalates to Critical.
    /// The config guarantees `threshold < critical` at load.
    pub critical: Decimal,
}

/// Detector for lopsided directional swap flow.
pub struct FlowImbalanceDetector {
    /// Source of per-pool directional USD volume.
    flow_repo: Arc<dyn SwapFlowRepository>,
    /// The protocol tag stamped on emitted signals. The flow repository is
    /// protocol-specific today (it reads the DAMM v2 view), so the detector
    /// is told which protocol it is observing at construction. When the flow
    /// read becomes cross-protocol, `PoolSwapFlow` will carry the protocol
    /// and this field goes away.
    protocol: Protocol,
    /// The detector's tuning knobs.
    settings: FlowImbalanceSettings,
}

impl FlowImbalanceDetector {
    /// Build the detector: its dependencies positionally, its tuning as a
    /// named-field struct.
    pub fn new(
        flow_repo: Arc<dyn SwapFlowRepository>,
        protocol: Protocol,
        settings: FlowImbalanceSettings,
    ) -> Self {
        Self {
            flow_repo,
            protocol,
            settings,
        }
    }
}

#[async_trait]
impl SignalDetector for FlowImbalanceDetector {
    fn name(&self) -> &'static str {
        "flow_imbalance"
    }

    fn interval(&self) -> Duration {
        self.settings.interval
    }

    fn cooldown(&self) -> Duration {
        self.settings.cooldown
    }

    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        let since = ctx.evaluated_at - self.settings.window;
        let flows = self.flow_repo.directional_volume_since(since).await?;

        let mut signals = Vec::new();
        for flow in flows {
            let total = flow.volume_a_to_b_usd + flow.volume_b_to_a_usd;
            // Volume floor: skip pools too thin for the ratio to mean anything
            // (also guards the division below against a zero denominator).
            if total < self.settings.min_volume_usd || total.is_zero() {
                continue;
            }

            let imbalance = (flow.volume_a_to_b_usd - flow.volume_b_to_a_usd) / total;
            let magnitude = imbalance.abs();
            if magnitude < self.settings.threshold {
                continue;
            }

            let severity = if magnitude >= self.settings.critical {
                Severity::Critical
            } else {
                Severity::Warning
            };

            signals.push(Signal {
                detector: self.name().to_string(),
                protocol: self.protocol,
                pool_address: flow.pool_address,
                severity,
                value: imbalance,
                threshold: Some(self.settings.threshold),
                message: Some(format!(
                    "directional flow imbalance {} (total volume ${})",
                    imbalance.round_dp(4),
                    total.round_dp(2),
                )),
                triggered_at: ctx.evaluated_at,
            });
        }

        Ok(signals)
    }
}

#[cfg(test)]
#[path = "flow_imbalance_tests.rs"]
mod tests;
