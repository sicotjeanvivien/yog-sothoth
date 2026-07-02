//! Price-oracle deviation detector.
//!
//! Compares each pool's on-chain **spot price** (decoded from the Q64.64
//! `sqrt_price` of its last swap) with the **oracle price** implied by the
//! two tokens' latest USD observations, and emits a signal when the relative
//! gap
//!
//! ```text
//!   deviation = (spot_a_in_b - oracle_a_in_b) / oracle_a_in_b
//! ```
//!
//! crosses a threshold. Both sides are gated on freshness: a pool that has
//! not swapped recently carries a stale spot price, and a stale oracle price
//! (yog-context down) would read as a spurious deviation — either one makes
//! the comparison meaningless, so the pool is skipped, not signalled.
//!
//! Stateless between ticks: it recomputes from the DB snapshot each time.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use yog_core::domain::{
    DetectorError, EvalContext, PoolPriceSnapshot, PoolPriceSnapshotRepository, Protocol, Severity,
    Signal, SignalDetector,
};

/// Detector for pool spot price drifting away from the oracle price.
pub struct PriceOracleDeviationDetector {
    /// Source of per-pool spot-price inputs and latest oracle prices.
    snapshot_repo: Arc<dyn PoolPriceSnapshotRepository>,
    /// How often the engine ticks this detector.
    interval: Duration,
    /// Rolling per-pool suppression window (engine-level dedup).
    cooldown: Duration,
    /// Oldest acceptable oracle observation. Pools where either token's
    /// price is older are skipped.
    max_price_age: ChronoDuration,
    /// Oldest acceptable last swap. Pools quiet for longer are skipped —
    /// their spot price is history, not a live quote.
    max_spot_age: ChronoDuration,
    /// `|deviation|` at or above which a signal is emitted.
    threshold: Decimal,
}

impl PriceOracleDeviationDetector {
    pub fn new(
        snapshot_repo: Arc<dyn PoolPriceSnapshotRepository>,
        interval: Duration,
        cooldown: Duration,
        max_price_age: ChronoDuration,
        max_spot_age: ChronoDuration,
        threshold: Decimal,
    ) -> Self {
        Self {
            snapshot_repo,
            interval,
            cooldown,
            max_price_age,
            max_spot_age,
            threshold,
        }
    }
}

/// Decode a snapshot's raw `sqrt_price` into a spot price (token B per
/// 1 token A, human units). The interpretation is protocol-specific; a
/// protocol without a decoder yields `None` and the pool is skipped.
fn spot_price_a_in_b(snapshot: &PoolPriceSnapshot) -> Option<Decimal> {
    match snapshot.protocol {
        Protocol::MeteoraDammV2 => yog_core::amm::damm_v2::sqrt_price_to_price_a_in_b(
            snapshot.sqrt_price,
            snapshot.decimals_a,
            snapshot.decimals_b,
        ),
        // No sqrt_price decoder for these yet — and the indexer does not
        // ingest them either, so no snapshot carries them today.
        Protocol::MeteoraDammV1 | Protocol::MeteoraDlmm => None,
    }
}

/// Render a price for the message without drowning it in digits — six
/// significant figures keeps memecoin magnitudes (1e-12) readable where a
/// fixed decimal rounding would collapse them to zero.
fn compact(price: Decimal) -> Decimal {
    price.round_sf(6).unwrap_or(price).normalize()
}

#[async_trait]
impl SignalDetector for PriceOracleDeviationDetector {
    fn name(&self) -> &'static str {
        "price_oracle_deviation"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn cooldown(&self) -> Duration {
        self.cooldown
    }

    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        let snapshots = self.snapshot_repo.latest().await?;

        let price_cutoff = ctx.evaluated_at - self.max_price_age;
        let spot_cutoff = ctx.evaluated_at - self.max_spot_age;
        // `|deviation|` at or above which the pool price is way off → Critical.
        let critical_deviation = Decimal::new(2, 1); // 0.2

        let mut signals = Vec::new();
        for snapshot in snapshots {
            // Freshness gates: a stale side makes the comparison
            // meaningless, not alarming.
            if snapshot.last_swap_at < spot_cutoff
                || snapshot.price_a_fetched_at < price_cutoff
                || snapshot.price_b_fetched_at < price_cutoff
            {
                continue;
            }

            // Oracle price of A in B units. checked_div covers both the
            // zero-price row and a magnitude overflow on extreme pairs.
            let Some(oracle) = snapshot.price_a_usd.checked_div(snapshot.price_b_usd) else {
                continue;
            };
            if oracle <= Decimal::ZERO {
                continue;
            }

            let Some(spot) = spot_price_a_in_b(&snapshot) else {
                continue;
            };

            let Some(deviation) = (spot - oracle).checked_div(oracle) else {
                continue;
            };
            let magnitude = deviation.abs();
            if magnitude < self.threshold {
                continue;
            }

            let severity = if magnitude >= critical_deviation {
                Severity::Critical
            } else {
                Severity::Warning
            };

            signals.push(Signal {
                detector: self.name().to_string(),
                protocol: snapshot.protocol,
                pool_address: snapshot.pool_address,
                severity,
                value: deviation,
                threshold: Some(self.threshold),
                message: Some(format!(
                    "spot price deviates {} from oracle (spot {}, oracle {})",
                    deviation.round_dp(4),
                    compact(spot),
                    compact(oracle),
                )),
                triggered_at: ctx.evaluated_at,
            });
        }

        Ok(signals)
    }
}

#[cfg(test)]
#[path = "price_oracle_deviation_tests.rs"]
mod tests;
