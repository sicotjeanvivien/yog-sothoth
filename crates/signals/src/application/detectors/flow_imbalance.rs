//! Flow imbalance detector: over a trailing window, `(a_to_b_usd - b_to_a_usd) / (a_to_b_usd +
//! b_to_a_usd)` ∈ [-1, +1], from [`SwapFlowRepository`]. Stateless between ticks.
//! ⚠️ Only above a volume floor: a near-dead pool with two dust swaps would read a spurious ±1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use yog_core::domain::{
    DetectorError, EvalContext, Protocol, Severity, Signal, SignalDetector, SwapFlowRepository,
};

use super::metrics::{DetectorMetrics, SkipReason};

/// Tuning knobs of the flow-imbalance detector, from the bootstrap config; named fields, since
/// three `Decimal`s and three durations would let a swapped pair compile.
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
    /// `|imbalance|` at or above which the signal escalates to Critical; above `threshold`, by config.
    pub critical: Decimal,
}

/// Detector for lopsided directional swap flow.
pub struct FlowImbalanceDetector {
    /// Source of per-pool directional USD volume.
    flow_repo: Arc<dyn SwapFlowRepository>,
    /// The protocol tag stamped on emitted signals: the flow read is protocol-specific (DAMM v2 view).
    protocol: Protocol,
    /// The detector's tuning knobs.
    settings: FlowImbalanceSettings,
}

impl FlowImbalanceDetector {
    /// Build the detector: dependencies positionally, tuning as a named-field struct.
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
        DetectorMetrics::record_considered(self.name(), flows.len());

        let mut signals = Vec::new();
        for flow in flows {
            // Valuation guard first.
            let (Some(a_to_b), Some(b_to_a)) = (flow.volume_a_to_b_usd, flow.volume_b_to_a_usd)
            else {
                DetectorMetrics::record_skipped(self.name(), SkipReason::Unpriced);
                continue;
            };

            let total = a_to_b + b_to_a;
            // Volume floor: too thin for the ratio to mean anything; also guards the division.
            if total < self.settings.min_volume_usd || total.is_zero() {
                continue;
            }

            let imbalance = (a_to_b - b_to_a) / total;
            let magnitude = imbalance.abs();
            if magnitude < self.settings.threshold {
                continue;
            }

            // The recorded threshold is the one that justifies the severity, the critical one for a
            // Critical signal, not the emission floor.
            let (severity, threshold) = if magnitude >= self.settings.critical {
                (Severity::Critical, self.settings.critical)
            } else {
                (Severity::Warning, self.settings.threshold)
            };

            signals.push(Signal {
                detector: self.name().to_string(),
                protocol: self.protocol,
                pool_address: flow.pool_address,
                severity,
                value: imbalance,
                threshold: Some(threshold),
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
