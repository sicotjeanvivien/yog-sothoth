//! TVL drain detector: over a trailing window, `drain = net_removed / (tvl_usd + net_removed)`,
//! with `net_removed = removed_usd - added_usd`, from [`LiquidityFlowRepository`]. Stateless.
//! ⚠️ The floor and the ratio are against the *starting* TVL, so a drained pool cannot slip under
//! the floor by the drain itself. An unvaluable pool is skipped: no signal beats a fake one.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use yog_core::domain::{
    DETECTOR_TVL_DRAIN, DetectorError, EvalContext, LiquidityFlowRepository, Protocol, Severity,
    Signal, SignalDetector,
};

use super::metrics::{DetectorMetrics, SkipReason};

/// Tuning knobs of the TVL-drain detector, from the bootstrap config; named fields, since three
/// `Decimal`s and three durations would let a swapped pair compile.
pub struct TvlDrainSettings {
    /// Trailing window over which liquidity flow is aggregated.
    pub window: ChronoDuration,
    /// How often the engine ticks this detector.
    pub interval: Duration,
    /// Rolling per-pool suppression window (engine-level dedup).
    pub cooldown: Duration,
    /// Minimum starting TVL (current TVL + net removed) for a pool to be considered.
    pub min_tvl_usd: Decimal,
    /// Drain ratio at or above which a signal is emitted.
    pub threshold: Decimal,
    /// Drain ratio at or above which the signal escalates to Critical; above `threshold`, by config.
    pub critical: Decimal,
}

/// Detector for pools being emptied of their liquidity.
pub struct TvlDrainDetector {
    /// Source of per-pool windowed liquidity flow + current TVL.
    flow_repo: Arc<dyn LiquidityFlowRepository>,
    /// The protocol tag stamped on emitted signals: the read is protocol-specific (DAMM v2 view).
    protocol: Protocol,
    /// The detector's tuning knobs.
    settings: TvlDrainSettings,
}

impl TvlDrainDetector {
    /// Build the detector: dependencies positionally, tuning as a named-field struct.
    pub fn new(
        flow_repo: Arc<dyn LiquidityFlowRepository>,
        protocol: Protocol,
        settings: TvlDrainSettings,
    ) -> Self {
        Self {
            flow_repo,
            protocol,
            settings,
        }
    }
}

#[async_trait]
impl SignalDetector for TvlDrainDetector {
    fn name(&self) -> &'static str {
        DETECTOR_TVL_DRAIN
    }

    fn interval(&self) -> Duration {
        self.settings.interval
    }

    fn cooldown(&self) -> Duration {
        self.settings.cooldown
    }

    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        let since = ctx.evaluated_at - self.settings.window;
        let flows = self.flow_repo.liquidity_flow_since(since).await?;
        DetectorMetrics::record_considered(self.name(), flows.len());

        let mut signals = Vec::new();
        for flow in flows {
            // TVL guard: an unvaluable pool is counted, "cannot see this pool" not being "no drain".
            let Some(tvl) = flow.tvl_usd else {
                DetectorMetrics::record_skipped(self.name(), SkipReason::NoTvl);
                continue;
            };

            // ⚠️ Flow guard: a partly valuable window is a sub-total, which would under-estimate the
            // drain in silence.
            let (Some(added), Some(removed)) = (flow.added_usd, flow.removed_usd) else {
                DetectorMetrics::record_skipped(self.name(), SkipReason::Unpriced);
                continue;
            };

            // LP churn (rebalancing) nets out; net inflow is a healthy pool.
            let net_removed = removed - added;
            if net_removed <= Decimal::ZERO {
                continue;
            }

            // ⚠️ The floor is on the starting TVL, so a pool emptied within the window cannot dodge
            // it; it also guards the division (net_removed > 0 ⇒ starting_tvl > 0).
            let starting_tvl = tvl + net_removed;
            if starting_tvl < self.settings.min_tvl_usd {
                continue;
            }

            let drain = net_removed / starting_tvl;
            if drain < self.settings.threshold {
                continue;
            }

            // The recorded threshold is the one that justifies the severity, the critical one for a
            // Critical signal, not the emission floor.
            let (severity, threshold) = if drain >= self.settings.critical {
                (Severity::Critical, self.settings.critical)
            } else {
                (Severity::Warning, self.settings.threshold)
            };

            signals.push(Signal {
                detector: self.name().to_string(),
                protocol: self.protocol,
                pool_address: flow.pool_address,
                severity,
                value: drain,
                threshold: Some(threshold),
                message: Some(format!(
                    "liquidity drain {} (net ${} removed, TVL now ${})",
                    drain.round_dp(4),
                    net_removed.round_dp(2),
                    tvl.round_dp(2),
                )),
                triggered_at: ctx.evaluated_at,
            });
        }

        Ok(signals)
    }
}

#[cfg(test)]
#[path = "tvl_drain_tests.rs"]
mod tests;
