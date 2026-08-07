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
        volume_a_to_b_usd: Some(usd(a2b)),
        volume_b_to_a_usd: Some(usd(b2a)),
    }
}

/// A pool whose window could not be fully valued — some hour carried an
/// amount in a token with no usable price. Both directions arrive absent
/// together; that is the shape the repository now produces.
fn unvaluable_flow(seed: u8) -> PoolSwapFlow {
    PoolSwapFlow {
        pool_address: pk(seed),
        volume_a_to_b_usd: None,
        volume_b_to_a_usd: None,
    }
}

/// Build a detector: window 24h, interval 300s, cooldown 6h, floor $1000,
/// Build a detector over the mock. (Cooldown is engine-level, so it
/// doesn't affect `evaluate`.)
fn detector(flows: Vec<PoolSwapFlow>) -> FlowImbalanceDetector {
    FlowImbalanceDetector::new(
        Arc::new(MockFlowRepo(flows)),
        Protocol::MeteoraDammV2,
        FlowImbalanceSettings {
            window: ChronoDuration::hours(24),
            interval: Duration::from_secs(300),
            cooldown: Duration::from_secs(6 * 3600),
            min_volume_usd: usd(1000),
            threshold: Decimal::new(3, 1), // 0.3
            critical: Decimal::new(9, 1),  // 0.9
        },
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
    // A critical signal records the *critical* boundary, not the
    // emission floor — the threshold must justify the severity.
    assert_eq!(signals[0].threshold, Some(Decimal::new(9, 1)));
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

// ── The defect `.project` ticket 08 exists to remove ─────────────────────────

#[tokio::test]
async fn an_unvaluable_window_emits_nothing_rather_than_a_critical() {
    // The test the ticket asks for by name. Before the fix, a window whose
    // token A had no usable price arrived with that direction COALESCEd to 0
    // while the other kept its full value — so this pool produced
    // `(0 − X)/X = −1.0` exactly: a Critical at maximum magnitude, on a pool
    // that may be perfectly balanced and merely half-unseen.
    //
    // Mutation check: give `unvaluable_flow` a `Some(usd(0))` on one direction
    // and a real value on the other, and this goes red with one Critical at
    // -1.0 — which is precisely what the old COALESCE produced.
    let signals = run(&detector(vec![unvaluable_flow(1)])).await;
    assert!(
        signals.is_empty(),
        "an unvaluable window is unknown, not one-sided; got {signals:?}"
    );
}

#[tokio::test]
async fn an_unvaluable_pool_does_not_suppress_its_valuable_neighbours() {
    // The skip is per pool. A batch must not lose its good pools because one
    // of them could not be valued — that would trade a false positive for a
    // false negative on everything else.
    let signals = run(&detector(vec![
        unvaluable_flow(1),
        flow(2, 10_000, 0), // 1.0 → Critical
    ]))
    .await;
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].pool_address, pk(2));
    assert_eq!(signals[0].severity, Severity::Critical);
}
