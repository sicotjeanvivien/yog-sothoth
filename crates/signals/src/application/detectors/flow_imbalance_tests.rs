//! Unit tests for the flow-imbalance math and threshold, and the volume floor handed to the read.
//! DB-free: a hand-written mock `SwapFlowRepository` feeds fixed flows.

use std::sync::Mutex;

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

/// Mock returning a fixed set of flows, ignoring `since` and recording the floor it was asked for.
struct MockFlowRepo {
    flows: Vec<PoolSwapFlow>,
    floors: Mutex<Vec<Decimal>>,
}

impl MockFlowRepo {
    fn new(flows: Vec<PoolSwapFlow>) -> Arc<Self> {
        Arc::new(Self {
            flows,
            floors: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl SwapFlowRepository for MockFlowRepo {
    async fn directional_volume_since(
        &self,
        _since: chrono::DateTime<Utc>,
        min_volume_usd: Decimal,
    ) -> RepositoryResult<Vec<PoolSwapFlow>> {
        self.floors.lock().unwrap().push(min_volume_usd);
        Ok(self.flows.clone())
    }
}

fn flow(seed: u8, a2b: i64, b2a: i64) -> PoolSwapFlow {
    PoolSwapFlow {
        pool_address: pk(seed),
        volume_a_to_b_usd: Some(usd(a2b)),
        volume_b_to_a_usd: Some(usd(b2a)),
    }
}

/// A pool whose window could not be fully valued: both directions arrive absent together.
fn unvaluable_flow(seed: u8) -> PoolSwapFlow {
    PoolSwapFlow {
        pool_address: pk(seed),
        volume_a_to_b_usd: None,
        volume_b_to_a_usd: None,
    }
}

/// Build a detector over `repo`; the cooldown is engine-level and does not affect `evaluate`.
fn detector_with_floor(repo: Arc<MockFlowRepo>, min_volume_usd: Decimal) -> FlowImbalanceDetector {
    FlowImbalanceDetector::new(
        repo,
        Protocol::MeteoraDammV2,
        FlowImbalanceSettings {
            window: ChronoDuration::hours(24),
            interval: Duration::from_secs(300),
            cooldown: Duration::from_secs(6 * 3600),
            min_volume_usd,
            threshold: Decimal::new(3, 1), // 0.3
            critical: Decimal::new(9, 1),  // 0.9
        },
    )
}

fn detector(flows: Vec<PoolSwapFlow>) -> FlowImbalanceDetector {
    detector_with_floor(MockFlowRepo::new(flows), usd(1000))
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
    // A critical signal records the critical boundary, not the emission floor.
    assert_eq!(signals[0].threshold, Some(Decimal::new(9, 1)));
}

#[tokio::test]
async fn the_configured_volume_floor_is_handed_to_the_read() {
    // Mutation: the read handed a constant instead of `min_volume_usd`.
    let repo = MockFlowRepo::new(Vec::new());
    run(&detector_with_floor(Arc::clone(&repo), usd(1234))).await;
    assert_eq!(*repo.floors.lock().unwrap(), vec![usd(1234)]);
}

#[tokio::test]
async fn a_zero_total_under_a_zero_floor_emits_nothing() {
    // Mutation: the `total.is_zero()` guard removed — `Decimal` division by zero panics.
    let det = detector_with_floor(MockFlowRepo::new(vec![flow(1, 0, 0)]), Decimal::ZERO);
    assert!(run(&det).await.is_empty());
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
    ]))
    .await;
    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].pool_address, pk(2));
    assert_eq!(signals[0].severity, Severity::Warning);
    assert_eq!(signals[1].pool_address, pk(3));
    assert_eq!(signals[1].severity, Severity::Critical);
}

// ── an unvaluable window ───────────────────────────────────────────────

#[tokio::test]
async fn an_unvaluable_window_emits_nothing_rather_than_a_critical() {
    let signals = run(&detector(vec![unvaluable_flow(1)])).await;
    assert!(
        signals.is_empty(),
        "an unvaluable window is unknown, not one-sided; got {signals:?}"
    );
}

#[tokio::test]
async fn an_unvaluable_pool_does_not_suppress_its_valuable_neighbours() {
    // The skip is per pool: a batch keeps its good pools when one cannot be valued.
    let signals = run(&detector(vec![
        unvaluable_flow(1),
        flow(2, 10_000, 0), // 1.0 → Critical
    ]))
    .await;
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].pool_address, pk(2));
    assert_eq!(signals[0].severity, Severity::Critical);
}

// ── the skip is a number, not merely an absence of signal ──────────────────
//
// ⚠️ A detector that stopped working also emits nothing: the counters tell "nothing there" from
// "could not look". Not `#[tokio::test]`: `with_local_recorder` binds the current thread.

use crate::application::metrics_probe::{counter, snapshot};

/// Evaluate once under a thread-local recorder and return its snapshot.
fn snapshot_one_evaluation(
    det: FlowImbalanceDetector,
) -> crate::application::metrics_probe::Snapshot {
    snapshot(|| async move {
        run(&det).await;
    })
}

#[test]
fn an_unvaluable_pool_is_counted_not_just_silently_dropped() {
    let snapshot = snapshot_one_evaluation(detector(vec![
        unvaluable_flow(1),
        unvaluable_flow(2),
        flow(3, 10_000, 0), // valuable, and lopsided enough to signal
    ]));

    assert_eq!(
        counter(
            &snapshot,
            "yog_signals_considered_total",
            &[("detector", "flow_imbalance")]
        ),
        Some(3),
        "all three pools were handed to the detector — this is the denominator \
         without which the skip count says nothing"
    );
    assert_eq!(
        counter(
            &snapshot,
            "yog_signals_skipped_total",
            &[("detector", "flow_imbalance"), ("reason", "unpriced")]
        ),
        Some(2),
        "two of them could not be valued, and the metric has to say so — \
         'no signal' and 'cannot see' must not read alike on a dashboard"
    );
}
