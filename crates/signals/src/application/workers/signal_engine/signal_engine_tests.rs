//! Unit tests for the dedup filter, for one tick and for the stop, against a fake detector and repository.

use std::sync::Mutex;

use super::*;
use crate::application::metrics_probe::{Snapshot, counter, snapshot};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::RepositoryError;
use yog_core::domain::{DetectorError, Protocol};

const NAME: &str = "flow_imbalance";

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn sig(pool: u8, severity: Severity) -> Signal {
    Signal {
        detector: NAME.to_string(),
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

/// A detector whose tick ends at once, never ends, panics, or panics a second after it began.
struct Detector {
    name: &'static str,
    tick: Tick,
}

#[derive(Clone, Copy)]
enum Tick {
    Returns,
    NeverReturns,
    Panics,
    PanicsAfterASecond,
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
    async fn evaluate(&self, _ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        match self.tick {
            Tick::Returns => Ok(Vec::new()),
            Tick::NeverReturns => std::future::pending().await,
            Tick::Panics => panic!("the detector's tick panicked"),
            Tick::PanicsAfterASecond => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                panic!("the detector's tick panicked during the stop")
            }
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
        _since: DateTime<Utc>,
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

/// Mutation: the drain waiting out the grace whatever happens — an ordinary stop then lasts the whole grace.
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
#[tokio::test(start_paused = true)]
async fn a_detector_that_panics_stops_the_engine() {
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

    let result = engine.run(CancellationToken::new()).await;

    assert!(
        matches!(result, Err(EngineError::DetectorPanicked(_))),
        "{result:?}"
    );
}

/// Mutation: a panic during the stop ending the drain at once — the detector still stuck is then never
/// named, which is the case the grace exists for.
#[tokio::test(start_paused = true)]
async fn a_panic_during_the_stop_still_waits_for_the_grace_then_fails() {
    let (result, waited) = stop_after_first_ticks(&[
        ("stuck", Tick::NeverReturns),
        ("broken", Tick::PanicsAfterASecond),
    ])
    .await;

    assert!(
        matches!(result, Err(EngineError::DetectorPanicked(_))),
        "{result:?}"
    );
    assert_eq!(waited, SHUTDOWN_GRACE);
}

const COOLDOWN_SECS: u64 = 90;

/// A detector with one answer, `None` being a failure, which notes the date it was asked for.
struct ScriptedDetector {
    answer: Option<Vec<Signal>>,
    evaluated_at: Mutex<Option<DateTime<Utc>>>,
}

impl ScriptedDetector {
    fn new(answer: Option<Vec<Signal>>) -> Self {
        Self {
            answer,
            evaluated_at: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl SignalDetector for ScriptedDetector {
    fn name(&self) -> &'static str {
        NAME
    }
    // ⚠️ Differs from the cooldown, so that reading one for the other fails.
    fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }
    fn cooldown(&self) -> std::time::Duration {
        std::time::Duration::from_secs(COOLDOWN_SECS)
    }
    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError> {
        *self.evaluated_at.lock().expect("evaluated_at") = Some(ctx.evaluated_at);
        self.answer
            .clone()
            .ok_or_else(|| DetectorError::Evaluation("scripted".to_string()))
    }
}

/// A repository with one answer per call, `None` and `write_fails` being failures, which notes
/// every lookup and every attempted write.
struct ScriptedRepository {
    recent: Option<HashMap<Pubkey, Severity>>,
    write_fails: bool,
    lookups: Mutex<Vec<(String, DateTime<Utc>)>>,
    writes: Mutex<Vec<Vec<Signal>>>,
}

impl ScriptedRepository {
    fn new(recent: Option<HashMap<Pubkey, Severity>>, write_fails: bool) -> Self {
        Self {
            recent,
            write_fails,
            lookups: Mutex::default(),
            writes: Mutex::default(),
        }
    }
    fn knowing(recent: HashMap<Pubkey, Severity>) -> Self {
        Self::new(Some(recent), false)
    }
    fn lookups(&self) -> usize {
        self.lookups.lock().expect("lookups").len()
    }
    fn writes(&self) -> Vec<Vec<Signal>> {
        self.writes.lock().expect("writes").clone()
    }
}

#[async_trait::async_trait]
impl SignalRepository for ScriptedRepository {
    async fn insert_batch(&self, signals: &[Signal]) -> yog_core::RepositoryResult<()> {
        self.writes.lock().expect("writes").push(signals.to_vec());
        if self.write_fails {
            return Err(RepositoryError::Backend("scripted".to_string()));
        }
        Ok(())
    }
    async fn latest_severity_by_pool(
        &self,
        detector: &str,
        since: DateTime<Utc>,
    ) -> yog_core::RepositoryResult<HashMap<Pubkey, Severity>> {
        self.lookups
            .lock()
            .expect("lookups")
            .push((detector.to_string(), since));
        self.recent
            .clone()
            .ok_or_else(|| RepositoryError::Backend("scripted".to_string()))
    }
}

fn tick(detector: &ScriptedDetector, repository: &ScriptedRepository) -> Snapshot {
    snapshot(|| async { run_tick(detector, repository, NAME).await })
}

/// `outcome` counted once and no other, with `emitted` signals counted, `None` being never touched.
fn assert_counted(counted: &Snapshot, outcome: &str, emitted: Option<u64>) {
    let ticks: Vec<_> = counted
        .iter()
        .filter(|(key, _, _, _)| key.key().name() == "yog_signals_tick_total")
        .collect();
    assert_eq!(ticks.len(), 1, "{ticks:?}");
    assert_eq!(
        counter(
            counted,
            "yog_signals_tick_total",
            &[("detector", NAME), ("outcome", outcome)],
        ),
        Some(1)
    );
    assert_eq!(
        counter(counted, "yog_signals_emitted_total", &[("detector", NAME)]),
        emitted
    );
}

/// Mutation: `eval_failed` counted as `ok`, or a failed evaluation read as no candidate.
#[test]
fn a_failed_evaluation_asks_nothing_and_writes_nothing() {
    let detector = ScriptedDetector::new(None);
    let repository = ScriptedRepository::knowing(HashMap::new());

    let counted = tick(&detector, &repository);

    assert_eq!(repository.lookups(), 0);
    assert!(repository.writes().is_empty());
    assert_counted(&counted, "eval_failed", None);
}

/// Mutation: the early return on no candidate removed, which then counts `suppressed`.
#[test]
fn no_candidate_asks_nothing_and_counts_an_ordinary_tick() {
    let detector = ScriptedDetector::new(Some(Vec::new()));
    let repository = ScriptedRepository::knowing(HashMap::new());

    let counted = tick(&detector, &repository);

    assert_eq!(repository.lookups(), 0);
    assert!(repository.writes().is_empty());
    assert_counted(&counted, "ok", None);
}

/// Mutation: the cooldown dropped from the date, the interval read for it, or the date taken anew.
#[test]
fn the_lookup_receives_the_detector_and_the_start_of_its_cooldown() {
    let detector = ScriptedDetector::new(Some(vec![sig(1, Severity::Warning)]));
    let repository = ScriptedRepository::knowing(HashMap::new());

    tick(&detector, &repository);

    let evaluated_at = detector
        .evaluated_at
        .lock()
        .expect("evaluated_at")
        .expect("the detector was evaluated");
    let lookups = repository.lookups.lock().expect("lookups");
    assert_eq!(
        *lookups,
        [(
            NAME.to_string(),
            evaluated_at - ChronoDuration::seconds(COOLDOWN_SECS as i64)
        )]
    );
}

/// Mutation: `dedup_failed` and `persist_failed` swapped, or a failed lookup read as no past signal,
/// which would write a signal twice.
#[test]
fn a_failed_lookup_writes_nothing_and_is_counted_under_its_reason() {
    let detector = ScriptedDetector::new(Some(vec![sig(1, Severity::Warning)]));
    let repository = ScriptedRepository::new(None, false);

    let counted = tick(&detector, &repository);

    assert_eq!(repository.lookups(), 1);
    assert!(repository.writes().is_empty());
    assert_counted(&counted, "dedup_failed", None);
}

/// Mutation: `suppressed` counted as `ok`, or an empty batch sent to the repository.
#[test]
fn candidates_all_within_the_cooldown_write_nothing_and_count_as_suppressed() {
    let detector = ScriptedDetector::new(Some(vec![sig(1, Severity::Warning)]));
    let repository = ScriptedRepository::knowing(HashMap::from([(pk(1), Severity::Warning)]));

    let counted = tick(&detector, &repository);

    assert!(repository.writes().is_empty());
    assert_counted(&counted, "suppressed", None);
}

/// Mutation: `candidates` written instead of `to_emit`, a signal altered before the write, or the
/// count taken from the candidates.
#[test]
fn only_the_retained_candidates_are_written_and_counted() {
    let new_pool = sig(2, Severity::Warning);
    let escalation = sig(3, Severity::Critical);
    let detector = ScriptedDetector::new(Some(vec![
        sig(1, Severity::Warning),
        new_pool.clone(),
        escalation.clone(),
    ]));
    let repository = ScriptedRepository::knowing(HashMap::from([
        (pk(1), Severity::Warning),
        (pk(3), Severity::Warning),
    ]));

    let counted = tick(&detector, &repository);

    assert_eq!(repository.writes(), [vec![new_pool, escalation]]);
    assert_counted(&counted, "ok", Some(2));
}

/// Mutation: `dedup_failed` and `persist_failed` swapped, or the signals counted before the write.
#[test]
fn a_failed_write_counts_no_signal_and_is_counted_under_its_reason() {
    let signal = sig(1, Severity::Warning);
    let detector = ScriptedDetector::new(Some(vec![signal.clone()]));
    let repository = ScriptedRepository::new(Some(HashMap::new()), true);

    let counted = tick(&detector, &repository);

    assert_eq!(repository.writes(), [vec![signal]]);
    assert_counted(&counted, "persist_failed", None);
}
