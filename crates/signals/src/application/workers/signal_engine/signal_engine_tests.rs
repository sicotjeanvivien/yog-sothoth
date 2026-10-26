//! Unit tests for the cooldown / escalation dedup filter; the `recent` map stands in for the repository.

use super::*;
use chrono::Utc;
use rust_decimal::Decimal;
use yog_core::domain::Protocol;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn sig(pool: u8, severity: Severity) -> Signal {
    Signal {
        detector: "flow_imbalance".to_string(),
        protocol: Protocol::MeteoraDammV2,
        pool_address: pk(pool),
        severity,
        value: Decimal::ONE,
        threshold: None,
        message: None,
        triggered_at: Utc::now(),
    }
}

fn pools(emitted: &[Signal]) -> Vec<u8> {
    emitted
        .iter()
        .map(|s| s.pool_address.to_bytes()[0])
        .collect()
}

#[test]
fn new_pool_is_emitted() {
    let recent = HashMap::new();
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert_eq!(pools(&out), vec![1]);
}

#[test]
fn same_severity_within_cooldown_is_suppressed() {
    let recent = HashMap::from([(pk(1), Severity::Warning)]);
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert!(out.is_empty());
}

#[test]
fn escalation_breaks_the_cooldown() {
    // Already emitted Warning; a Critical for the same pool must get through.
    let recent = HashMap::from([(pk(1), Severity::Warning)]);
    let out = emittable(vec![sig(1, Severity::Critical)], &recent);
    assert_eq!(pools(&out), vec![1]);
    assert_eq!(out[0].severity, Severity::Critical);
}

#[test]
fn de_escalation_is_suppressed() {
    // Already Critical; a lower Warning must not re-alert.
    let recent = HashMap::from([(pk(1), Severity::Critical)]);
    let out = emittable(vec![sig(1, Severity::Warning)], &recent);
    assert!(out.is_empty());
}

#[test]
fn mixed_batch_keeps_only_new_and_escalating() {
    let recent = HashMap::from([
        (pk(1), Severity::Warning),  // pool 1: seen Warning
        (pk(2), Severity::Critical), // pool 2: seen Critical
    ]);
    let out = emittable(
        vec![
            sig(1, Severity::Warning),  // same → drop
            sig(2, Severity::Critical), // same → drop
            sig(3, Severity::Warning),  // new pool → keep
            sig(1, Severity::Critical), // escalation → keep
        ],
        &recent,
    );
    assert_eq!(pools(&out), vec![3, 1]);
}

// ── the stop: bounded by the shared grace, the detector that holds it named ────────────────

/// A detector whose tick ends at once, never ends, or panics.
struct Detector {
    name: &'static str,
    tick: Tick,
}

#[derive(Clone, Copy)]
enum Tick {
    Returns,
    NeverReturns,
    Panics,
}

#[async_trait::async_trait]
impl SignalDetector for Detector {
    fn name(&self) -> &'static str {
        self.name
    }
    fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }
    fn cooldown(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }
    async fn evaluate(
        &self,
        _ctx: &EvalContext,
    ) -> Result<Vec<Signal>, yog_core::domain::DetectorError> {
        match self.tick {
            Tick::Returns => Ok(Vec::new()),
            Tick::NeverReturns => std::future::pending().await,
            Tick::Panics => panic!("the detector's tick panicked"),
        }
    }
}

/// A repository with no past signal, which stores what it is given nowhere.
struct NoSignals;

#[async_trait::async_trait]
impl SignalRepository for NoSignals {
    async fn insert_batch(&self, _signals: &[Signal]) -> yog_core::RepositoryResult<()> {
        Ok(())
    }
    async fn latest_severity_by_pool(
        &self,
        _detector: &str,
        _since: chrono::DateTime<Utc>,
    ) -> yog_core::RepositoryResult<HashMap<Pubkey, Severity>> {
        Ok(HashMap::new())
    }
}

/// Starts the engine, lets every first tick begin, asks for the stop, and waits for `run`.
async fn stop_after_first_ticks(
    detectors: &[(&'static str, Tick)],
) -> (Result<Vec<&'static str>, EngineError>, std::time::Duration) {
    let engine = SignalEngine::new(
        Arc::new(NoSignals),
        detectors
            .iter()
            .map(|&(name, tick)| Arc::new(Detector { name, tick }) as Arc<dyn SignalDetector>)
            .collect(),
    );
    let shutdown = CancellationToken::new();
    let run = tokio::spawn(engine.run(shutdown.clone()));
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let asked = Instant::now();
    shutdown.cancel();
    let result = run.await.expect("the engine task itself does not panic");
    (result, asked.elapsed())
}

/// Mutation: the deadline dropped from the drain — this test then hangs, as the stuck tick never ends.
#[tokio::test(start_paused = true)]
async fn a_detector_that_never_returns_is_named_once_the_grace_runs_out() {
    let (result, waited) =
        stop_after_first_ticks(&[("quick", Tick::Returns), ("stuck", Tick::NeverReturns)]).await;

    assert_eq!(result.expect("a stuck detector is no failure"), ["stuck"]);
    assert_eq!(waited, SHUTDOWN_GRACE);
}

/// Mutation: the drain waiting out the grace whatever happens — an ordinary stop then lasts five seconds.
#[tokio::test(start_paused = true)]
async fn an_ordinary_stop_does_not_wait_for_the_grace() {
    let (result, waited) =
        stop_after_first_ticks(&[("quick", Tick::Returns), ("other", Tick::Returns)]).await;

    assert!(result.expect("an ordinary stop").is_empty());
    assert!(
        waited < std::time::Duration::from_secs(1),
        "waited {waited:?}"
    );
}

/// Mutation: a panic read as an ordinary end while the engine runs — the other detectors go on, the
/// engine never stops.
#[test]
fn a_detector_that_panics_stops_the_engine() {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(async {
            let engine = SignalEngine::new(
                Arc::new(NoSignals),
                vec![
                    Arc::new(Detector {
                        name: "quick",
                        tick: Tick::Returns,
                    }) as Arc<dyn SignalDetector>,
                    Arc::new(Detector {
                        name: "broken",
                        tick: Tick::Panics,
                    }),
                ],
            );
            engine.run(CancellationToken::new()).await
        });

    assert!(
        matches!(result, Err(EngineError::DetectorPanicked(_))),
        "{result:?}"
    );
}
