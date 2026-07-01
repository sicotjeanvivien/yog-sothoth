//! Unit tests for the flow-imbalance math, threshold and volume floor.
//! DB-free: a hand-written mock `SwapFlowRepository` feeds fixed flows.

use super::*;
use chrono::Utc;
use solana_pubkey::Pubkey;
use yog_core::RepositoryResult;
use yog_core::domain::PoolSwapFlow;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn usd(v: i64) -> Decimal {
    Decimal::from(v)
}

/// Mock returning a fixed set of flows, ignoring `since`.
struct MockFlowRepo(Vec<PoolSwapFlow>);

#[async_trait]
impl SwapFlowRepository for MockFlowRepo {
    async fn directional_volume_since(
        &self,
        _since: chrono::DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolSwapFlow>> {
        Ok(self.0.clone())
    }
}

fn flow(seed: u8, a2b: i64, b2a: i64) -> PoolSwapFlow {
    PoolSwapFlow {
        pool_address: pk(seed),
        volume_a_to_b_usd: usd(a2b),
        volume_b_to_a_usd: usd(b2a),
    }
}

/// Build a detector: window 24h, interval 300s, floor $1000, threshold 0.3.
fn detector(flows: Vec<PoolSwapFlow>) -> FlowImbalanceDetector {
    FlowImbalanceDetector::new(
        Arc::new(MockFlowRepo(flows)),
        Protocol::MeteoraDammV2,
        ChronoDuration::hours(24),
        Duration::from_secs(300),
        usd(1000),
        Decimal::new(3, 1), // 0.3
    )
}

async fn run(det: &FlowImbalanceDetector) -> Vec<Signal> {
    det.evaluate(&EvalContext {
        evaluated_at: Utc::now(),
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn balanced_pool_emits_nothing() {
    // (5000-5000)/10000 = 0 → below threshold.
    let signals = run(&detector(vec![flow(1, 5000, 5000)])).await;
    assert!(signals.is_empty());
}

#[tokio::test]
async fn moderate_imbalance_emits_warning() {
    // (7000-3000)/10000 = 0.4 ≥ 0.3, < 0.9 → Warning.
    let signals = run(&detector(vec![flow(1, 7000, 3000)])).await;
    assert_eq!(signals.len(), 1);
    let s = &signals[0];
    assert_eq!(s.detector, "flow_imbalance");
    assert_eq!(s.protocol, Protocol::MeteoraDammV2);
    assert_eq!(s.pool_address, pk(1));
    assert_eq!(s.severity, Severity::Warning);
    assert_eq!(s.value, Decimal::new(4, 1)); // 0.4
    assert_eq!(s.threshold, Some(Decimal::new(3, 1)));
}

#[tokio::test]
async fn one_sided_flow_is_critical_with_imbalance_one() {
    // (10000-0)/10000 = 1.0 ≥ 0.9 → Critical.
    let signals = run(&detector(vec![flow(1, 10_000, 0)])).await;
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].severity, Severity::Critical);
    assert_eq!(signals[0].value, Decimal::ONE);
}

#[tokio::test]
async fn below_volume_floor_is_skipped_even_if_lopsided() {
    // Total $100 < $1000 floor, despite a perfect one-sided flow.
    let signals = run(&detector(vec![flow(1, 100, 0)])).await;
    assert!(signals.is_empty());
}

#[tokio::test]
async fn just_below_threshold_is_skipped() {
    // (6000-4000)/10000 = 0.2 < 0.3.
    let signals = run(&detector(vec![flow(1, 6000, 4000)])).await;
    assert!(signals.is_empty());
}

#[tokio::test]
async fn mixed_batch_emits_only_qualifying_pools() {
    let signals = run(&detector(vec![
        flow(1, 5000, 5000), // balanced → no
        flow(2, 9000, 1000), // 0.8 → Warning
        flow(3, 10_000, 0),  // 1.0 → Critical
        flow(4, 50, 0),      // below floor → no
    ]))
    .await;
    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].pool_address, pk(2));
    assert_eq!(signals[0].severity, Severity::Warning);
    assert_eq!(signals[1].pool_address, pk(3));
    assert_eq!(signals[1].severity, Severity::Critical);
}
