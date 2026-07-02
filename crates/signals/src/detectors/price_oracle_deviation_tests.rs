//! Unit tests for the deviation math, freshness gates and severity split.
//! DB-free: a hand-written mock `PoolPriceSnapshotRepository` feeds fixed
//! snapshots.

use super::*;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use yog_core::RepositoryResult;

/// `sqrt_price` encoding a spot price of exactly 1.0 for equal decimals
/// (`(2^64 / 2^64)^2 = 1`).
const SQRT_PRICE_ONE: u128 = 1 << 64;
/// `sqrt_price` encoding a spot price of exactly 4.0 (`2^65 → ratio 2`).
const SQRT_PRICE_FOUR: u128 = 1 << 65;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Mock returning a fixed set of snapshots.
struct MockSnapshotRepo(Vec<PoolPriceSnapshot>);

#[async_trait]
impl PoolPriceSnapshotRepository for MockSnapshotRepo {
    async fn latest(&self) -> RepositoryResult<Vec<PoolPriceSnapshot>> {
        Ok(self.0.clone())
    }
}

/// A fresh, comparable snapshot: everything observed one minute before
/// `now`, both tokens at 6 decimals. Tests override what they probe.
fn snapshot(
    seed: u8,
    now: DateTime<Utc>,
    sqrt_price: u128,
    price_a: &str,
    price_b: &str,
) -> PoolPriceSnapshot {
    let fresh = now - ChronoDuration::minutes(1);
    PoolPriceSnapshot {
        pool_address: pk(seed),
        protocol: Protocol::MeteoraDammV2,
        sqrt_price,
        last_swap_at: fresh,
        decimals_a: 6,
        decimals_b: 6,
        price_a_usd: price_a.parse().unwrap(),
        price_a_fetched_at: fresh,
        price_b_usd: price_b.parse().unwrap(),
        price_b_fetched_at: fresh,
    }
}

/// Build a detector: interval 300s, cooldown 6h, price age 15min, spot age
/// 24h, threshold 0.05. (Cooldown is engine-level, so it doesn't affect
/// `evaluate`.)
fn detector(snapshots: Vec<PoolPriceSnapshot>) -> PriceOracleDeviationDetector {
    PriceOracleDeviationDetector::new(
        Arc::new(MockSnapshotRepo(snapshots)),
        Duration::from_secs(300),
        Duration::from_secs(6 * 3600),
        ChronoDuration::minutes(15),
        ChronoDuration::hours(24),
        Decimal::new(5, 2), // 0.05
    )
}

async fn run_at(det: &PriceOracleDeviationDetector, now: DateTime<Utc>) -> Vec<Signal> {
    det.evaluate(&EvalContext { evaluated_at: now })
        .await
        .unwrap()
}

#[tokio::test]
async fn no_signal_when_spot_matches_oracle() {
    let now = Utc::now();
    // Spot 1.0, oracle 2.0/2.0 = 1.0 → deviation 0.
    let det = detector(vec![snapshot(1, now, SQRT_PRICE_ONE, "2.0", "2.0")]);
    assert!(run_at(&det, now).await.is_empty());
}

#[tokio::test]
async fn no_signal_below_threshold() {
    let now = Utc::now();
    // Spot 1.0, oracle 1.02 → deviation ≈ -0.0196 < 0.05.
    let det = detector(vec![snapshot(1, now, SQRT_PRICE_ONE, "1.02", "1.0")]);
    assert!(run_at(&det, now).await.is_empty());
}

#[tokio::test]
async fn warning_with_signed_deviation_when_threshold_crossed() {
    let now = Utc::now();
    // Spot 1.0, oracle 1.1 → deviation = -0.1/1.1 ≈ -0.0909.
    let det = detector(vec![snapshot(1, now, SQRT_PRICE_ONE, "1.1", "1.0")]);
    let signals = run_at(&det, now).await;

    assert_eq!(signals.len(), 1);
    let signal = &signals[0];
    assert_eq!(signal.detector, "price_oracle_deviation");
    assert_eq!(signal.pool_address, pk(1));
    assert_eq!(signal.severity, Severity::Warning);
    assert_eq!(signal.value.round_dp(4), Decimal::new(-909, 4));
    assert_eq!(signal.threshold, Some(Decimal::new(5, 2)));
    assert_eq!(signal.triggered_at, now);
}

#[tokio::test]
async fn critical_when_deviation_is_extreme() {
    let now = Utc::now();
    // Spot 4.0, oracle 1.0 → deviation 3.0 ≥ 0.2.
    let det = detector(vec![snapshot(1, now, SQRT_PRICE_FOUR, "1.0", "1.0")]);
    let signals = run_at(&det, now).await;

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].severity, Severity::Critical);
    assert_eq!(signals[0].value, Decimal::from(3));
}

#[tokio::test]
async fn skips_stale_oracle_price() {
    let now = Utc::now();
    // Would be Critical, but token B's price is older than max_price_age.
    let mut snap = snapshot(1, now, SQRT_PRICE_FOUR, "1.0", "1.0");
    snap.price_b_fetched_at = now - ChronoDuration::minutes(30);
    let det = detector(vec![snap]);
    assert!(run_at(&det, now).await.is_empty());
}

#[tokio::test]
async fn skips_stale_spot_price() {
    let now = Utc::now();
    // Would be Critical, but the pool hasn't swapped within max_spot_age.
    let mut snap = snapshot(1, now, SQRT_PRICE_FOUR, "1.0", "1.0");
    snap.last_swap_at = now - ChronoDuration::hours(48);
    let det = detector(vec![snap]);
    assert!(run_at(&det, now).await.is_empty());
}

#[tokio::test]
async fn skips_unpriceable_oracle() {
    let now = Utc::now();
    // A zero oracle price can't be deviated from.
    let det = detector(vec![snapshot(1, now, SQRT_PRICE_FOUR, "1.0", "0")]);
    assert!(run_at(&det, now).await.is_empty());
}

#[tokio::test]
async fn evaluates_each_pool_independently() {
    let now = Utc::now();
    let det = detector(vec![
        snapshot(1, now, SQRT_PRICE_ONE, "1.0", "1.0"), // in line → nothing
        snapshot(2, now, SQRT_PRICE_ONE, "1.1", "1.0"), // ~9% off → Warning
        snapshot(3, now, SQRT_PRICE_FOUR, "1.0", "1.0"), // 300% off → Critical
    ]);
    let signals = run_at(&det, now).await;

    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].pool_address, pk(2));
    assert_eq!(signals[0].severity, Severity::Warning);
    assert_eq!(signals[1].pool_address, pk(3));
    assert_eq!(signals[1].severity, Severity::Critical);
}
