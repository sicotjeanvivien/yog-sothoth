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
    /// Trailing window over which volume is aggregated.
    window: ChronoDuration,
    /// How often the engine ticks this detector.
    interval: Duration,
    /// Rolling per-pool suppression window (engine-level dedup).
    cooldown: Duration,
    /// Minimum total USD volume in the window for a pool to be considered.
    min_volume_usd: Decimal,
    /// `|imbalance|` at or above which a signal is emitted.
    threshold: Decimal,
}

impl FlowImbalanceDetector {
    pub fn new(
        flow_repo: Arc<dyn SwapFlowRepository>,
        protocol: Protocol,
        window: ChronoDuration,
        interval: Duration,
        cooldown: Duration,
        min_volume_usd: Decimal,
        threshold: Decimal,
    ) -> Self {
        Self {
            flow_repo,
            protocol,
            window,
            interval,
            cooldown,
            min_volume_usd,
            threshold,
        }
    }
}

#[async_trait]
impl SignalDetector for FlowImbalanceDetector {
    fn name(&self) -> &'static str {
        "flow_imbalance"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn cooldown(&self) -> Duration {
        self.cooldown
    }

    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        let since = ctx.evaluated_at - self.window;
        let flows = self.flow_repo.directional_volume_since(since).await?;

        // `|imbalance|` at or above which the flow is near one-sided → Critical.
        let critical_imbalance = Decimal::new(9, 1); // 0.9

        let mut signals = Vec::new();
        for flow in flows {
            let total = flow.volume_a_to_b_usd + flow.volume_b_to_a_usd;
            // Volume floor: skip pools too thin for the ratio to mean anything
            // (also guards the division below against a zero denominator).
            if total < self.min_volume_usd || total.is_zero() {
                continue;
            }

            let imbalance = (flow.volume_a_to_b_usd - flow.volume_b_to_a_usd) / total;
            let magnitude = imbalance.abs();
            if magnitude < self.threshold {
                continue;
            }

            let severity = if magnitude >= critical_imbalance {
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
                threshold: Some(self.threshold),
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
