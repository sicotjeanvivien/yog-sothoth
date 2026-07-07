//! TVL drain detector.
//!
//! Detects a pool being emptied of its liquidity (LP exodus, rug-like
//! behaviour): over a trailing window, what share of the starting TVL has
//! left? It reads per-pool windowed liquidity flow with current TVL
//! ([`LiquidityFlowRepository`]) and emits a signal when
//!
//! ```text
//!   net_removed = removed_usd - added_usd            (LP churn nets out)
//!   drain       = net_removed / (tvl_usd + net_removed)  ∈ (0, 1]
//! ```
//!
//! crosses a threshold. The denominator is the *starting* TVL (current TVL
//! plus what left), so the floor and the ratio are measured against the
//! pool as it was at the window's start — a pool drained to 5% of its size
//! must not slip under the floor because of the drain itself.
//!
//! Guards: a pool whose TVL cannot be valued (`tvl_usd = None` — unknown
//! price, unresolved mints, no reconstructed state) is skipped; no signal
//! beats a fake one. Net inflow (adds ≥ removes) is a healthy pool.
//!
//! Stateless between ticks: it recomputes from the DB snapshot each time.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use yog_core::domain::{
    DetectorError, EvalContext, LiquidityFlowRepository, Protocol, Severity, Signal, SignalDetector,
};

/// Tuning knobs of the TVL-drain detector, as loaded from the environment
/// by the bootstrap config. A named-field struct rather than constructor
/// arguments: half of these share a type (`Decimal` ×3, durations ×3), so
/// positional passing would let a swapped pair compile silently.
pub struct TvlDrainSettings {
    /// Trailing window over which liquidity flow is aggregated.
    pub window: ChronoDuration,
    /// How often the engine ticks this detector.
    pub interval: Duration,
    /// Rolling per-pool suppression window (engine-level dedup).
    pub cooldown: Duration,
    /// Minimum starting TVL (current TVL + net removed) for a pool to be
    /// considered — a dust pool losing its two dollars is not a drain.
    pub min_tvl_usd: Decimal,
    /// Drain ratio at or above which a signal is emitted.
    pub threshold: Decimal,
    /// Drain ratio at or above which the signal escalates to Critical.
    /// The config guarantees `threshold < critical` at load.
    pub critical: Decimal,
}

/// Detector for pools being emptied of their liquidity.
pub struct TvlDrainDetector {
    /// Source of per-pool windowed liquidity flow + current TVL.
    flow_repo: Arc<dyn LiquidityFlowRepository>,
    /// The protocol tag stamped on emitted signals. Same reasoning as the
    /// flow-imbalance detector: the read is protocol-specific today (DAMM
    /// v2 view), so the detector is told what it observes at construction.
    protocol: Protocol,
    /// The detector's tuning knobs.
    settings: TvlDrainSettings,
}

impl TvlDrainDetector {
    /// Build the detector: its dependencies positionally, its tuning as a
    /// named-field struct.
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
        "tvl_drain"
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

        let mut signals = Vec::new();
        for flow in flows {
            // TVL guard: an unvaluable pool yields no signal, not a fake one.
            let Some(tvl) = flow.tvl_usd else {
                continue;
            };

            // LP churn (rebalancing) nets out; net inflow is a healthy pool.
            let net_removed = flow.removed_usd - flow.added_usd;
            if net_removed <= Decimal::ZERO {
                continue;
            }

            // The floor is on the STARTING TVL — what the pool held before
            // the drain — so a pool emptied within the window can't dodge
            // the floor by having drained itself below it. Also guards the
            // division (net_removed > 0 ⇒ starting_tvl > 0 here).
            let starting_tvl = tvl + net_removed;
            if starting_tvl < self.settings.min_tvl_usd {
                continue;
            }

            let drain = net_removed / starting_tvl;
            if drain < self.settings.threshold {
                continue;
            }

            // The recorded threshold is the boundary that *justifies the
            // severity* — the critical one for a Critical signal — not the
            // emission floor, which would understate every escalation.
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
