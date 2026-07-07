//! Unit tests for the TVL-drain math, guards, threshold ladder and floor.
//! DB-free: a hand-written mock `LiquidityFlowRepository` feeds fixed flows.

use super::*;
use chrono::Utc;
use solana_pubkey::Pubkey;
use yog_core::RepositoryResult;
use yog_core::domain::PoolLiquidityFlow;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn usd(v: i64) -> Decimal {
    Decimal::from(v)
}

/// Mock returning a fixed set of flows, ignoring `since`.
struct MockFlowRepo(Vec<PoolLiquidityFlow>);

#[async_trait]
impl LiquidityFlowRepository for MockFlowRepo {
    async fn liquidity_flow_since(
        &self,
        _since: chrono::DateTime<Utc>,
    ) -> RepositoryResult<Vec<PoolLiquidityFlow>> {
        Ok(self.0.clone())
    }
}

fn flow(seed: u8, added: i64, removed: i64, tvl: Option<i64>) -> PoolLiquidityFlow {
    PoolLiquidityFlow {
        pool_address: pk(seed),
        added_usd: usd(added),
        removed_usd: usd(removed),
        tvl_usd: tvl.map(usd),
    }
}

/// Build a detector over the mock: floor $10k starting TVL, Warning 0.5,
/// Critical 0.8. (Cooldown is engine-level, so it doesn't affect
/// `evaluate`.)
fn detector(flows: Vec<PoolLiquidityFlow>) -> TvlDrainDetector {
    TvlDrainDetector::new(
        Arc::new(MockFlowRepo(flows)),
        Protocol::MeteoraDammV2,
        TvlDrainSettings {
            window: ChronoDuration::hours(6),
            interval: Duration::from_secs(300),
            cooldown: Duration::from_secs(6 * 3600),
            min_tvl_usd: usd(10_000),
            threshold: Decimal::new(5, 1), // 0.5
            critical: Decimal::new(8, 1),  // 0.8
        },
    )
}

async fn run(det: &TvlDrainDetector) -> Vec<Signal> {
    det.evaluate(&EvalContext {
        evaluated_at: Utc::now(),
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn drained_pool_emits_warning() {
    // Removed $60k net of $0 added, $40k left → drain 0.6 of $100k start.
    let det = detector(vec![flow(1, 0, 60_000, Some(40_000))]);
    let signals = run(&det).await;

    assert_eq!(signals.len(), 1);
    let s = &signals[0];
    assert_eq!(s.detector, "tvl_drain");
    assert_eq!(s.severity, Severity::Warning);
    assert_eq!(s.value, Decimal::new(6, 1));
    assert_eq!(s.threshold, Some(Decimal::new(5, 1)));
    assert_eq!(s.pool_address, pk(1));
}

#[tokio::test]
async fn heavy_drain_escalates_to_critical_with_its_own_threshold() {
    // $90k net removed of a $100k start → drain 0.9 ≥ critical 0.8.
    let det = detector(vec![flow(1, 0, 90_000, Some(10_000))]);
    let signals = run(&det).await;

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].severity, Severity::Critical);
    assert_eq!(signals[0].value, Decimal::new(9, 1));
    // The recorded threshold is the crossed critical boundary, not the floor.
    assert_eq!(signals[0].threshold, Some(Decimal::new(8, 1)));
}

#[tokio::test]
async fn churn_nets_out_below_threshold() {
    // $55k removed but $50k re-added (LP rebalancing): net $5k of a $105k
    // start → drain ≈ 0.048, silent.
    let det = detector(vec![flow(1, 50_000, 55_000, Some(100_000))]);
    assert!(run(&det).await.is_empty());
}

#[tokio::test]
async fn net_inflow_is_silent() {
    let det = detector(vec![flow(1, 80_000, 20_000, Some(100_000))]);
    assert!(run(&det).await.is_empty());
}

#[tokio::test]
async fn unvaluable_tvl_is_skipped() {
    // A drain-looking flow, but the pool cannot be valued: no signal
    // beats a fake one.
    let det = detector(vec![flow(1, 0, 60_000, None)]);
    assert!(run(&det).await.is_empty());
}

#[tokio::test]
async fn dust_pool_stays_below_the_floor() {
    // 90% drained, but the pool only ever held $1k — under the $10k floor.
    let det = detector(vec![flow(1, 0, 900, Some(100))]);
    assert!(run(&det).await.is_empty());
}

#[tokio::test]
async fn floor_measures_the_starting_tvl_not_the_remainder() {
    // $11k net removed, $1k left: the REMAINING TVL is dust but the
    // starting TVL ($12k) clears the floor — the drain itself must not
    // hide the pool. Drain 11/12 ≈ 0.917 → Critical.
    let det = detector(vec![flow(1, 0, 11_000, Some(1_000))]);
    let signals = run(&det).await;

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].severity, Severity::Critical);
}

#[tokio::test]
async fn fully_drained_pool_reads_one() {
    // Everything left, nothing remains: drain = 1.0 exactly.
    let det = detector(vec![flow(1, 0, 50_000, Some(0))]);
    let signals = run(&det).await;

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].value, Decimal::ONE);
    assert_eq!(signals[0].severity, Severity::Critical);
}

#[tokio::test]
async fn each_pool_is_judged_independently() {
    let det = detector(vec![
        flow(1, 0, 60_000, Some(40_000)),      // drain 0.6 → Warning
        flow(2, 40_000, 45_000, Some(95_000)), // net 5k / 100k → silent
        flow(3, 0, 90_000, Some(10_000)),      // drain 0.9 → Critical
    ]);
    let signals = run(&det).await;

    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].pool_address, pk(1));
    assert_eq!(signals[0].severity, Severity::Warning);
    assert_eq!(signals[1].pool_address, pk(3));
    assert_eq!(signals[1].severity, Severity::Critical);
}
