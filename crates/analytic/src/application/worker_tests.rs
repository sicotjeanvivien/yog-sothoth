//! The worker against a scripted repository and a heartbeat that records
//! what it is told.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::TimeZone;
use yog_bootstrap::RecordingHeartbeat;
use yog_core::RepositoryError;
use yog_core::domain::AnalyticProgress;

use super::*;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

const PROGRESS: AnalyticProgress = AnalyticProgress {
    first_hour: None,
    last_frozen: None,
    last_tranche: None,
};

/// Answers `progress` and `tvl` as scripted, writes every cycle unless `refusal` is set,
/// and keeps the cycles and the TVL measures it was asked for.
struct ScriptedRepository {
    progress: RepositoryResult<AnalyticProgress>,
    refusal: Option<RepositoryError>,
    tvl: RepositoryResult<TvlMeasure>,
    cycles: Mutex<Vec<CycleWrite>>,
    measures: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
}

#[async_trait]
impl AnalyticRepository for ScriptedRepository {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress> {
        self.progress.clone()
    }

    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome> {
        self.cycles.lock().unwrap().push(*cycle);
        match &self.refusal {
            Some(error) => Err(error.clone()),
            None => Ok(CycleOutcome {
                written: *cycle,
                tranches_written: 0,
                pool_hours: 0,
                hours_frozen: 0,
            }),
        }
    }

    async fn measure_tvl(
        &self,
        hour: DateTime<Utc>,
        at: DateTime<Utc>,
    ) -> RepositoryResult<TvlMeasure> {
        self.measures.lock().unwrap().push((hour, at));
        self.tvl.clone()
    }
}

type Scripted = (
    AnalyticWorker,
    Arc<ScriptedRepository>,
    Arc<RecordingHeartbeat>,
);

fn worker(
    progress: RepositoryResult<AnalyticProgress>,
    refusal: Option<RepositoryError>,
) -> Scripted {
    measuring(progress, refusal, Ok(TvlMeasure::Measured))
}

fn measuring(
    progress: RepositoryResult<AnalyticProgress>,
    refusal: Option<RepositoryError>,
    tvl: RepositoryResult<TvlMeasure>,
) -> Scripted {
    let repository = Arc::new(ScriptedRepository {
        progress,
        refusal,
        tvl,
        cycles: Mutex::new(Vec::new()),
        measures: Mutex::new(Vec::new()),
    });
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let worker = AnalyticWorker::new(
        repository.clone(),
        Some(heartbeat.clone()),
        Duration::from_secs(300),
    );
    (worker, repository, heartbeat)
}

fn signals(heartbeat: &RecordingHeartbeat) -> Vec<String> {
    heartbeat.signals.lock().unwrap().clone()
}

/// Against a cycle planned from another clock or another progress than the
/// ones the worker was given.
#[tokio::test]
async fn a_cycle_writes_what_the_plan_says_and_signals_success() {
    let progress = AnalyticProgress {
        first_hour: Some(at(9, 0)),
        last_frozen: Some(at(10, 0)),
        last_tranche: Some(at(10, 55)),
    };
    let (worker, repository, heartbeat) = worker(Ok(progress), None);

    worker.run_cycle(at(12, 30)).await.unwrap();

    assert_eq!(
        *repository.cycles.lock().unwrap(),
        [CycleWrite::plan(at(12, 30), progress)]
    );
    assert_eq!(signals(&heartbeat), ["success"]);
}

#[tokio::test]
async fn a_failed_write_signals_its_error() {
    let refused = RepositoryError::Backend("permission denied for table signals".into());
    let (worker, _, heartbeat) = worker(Ok(PROGRESS), Some(refused));

    assert!(worker.run_cycle(at(12, 30)).await.is_err());

    assert_eq!(signals(&heartbeat).len(), 1);
    assert!(
        signals(&heartbeat)[0].starts_with("failure: cycle failed:")
            && signals(&heartbeat)[0].contains("permission denied for table signals"),
        "{:?}",
        signals(&heartbeat)
    );
}

/// Against a cycle planned from a default progress when the read fails: it
/// would start the tables over at the current hour.
#[tokio::test]
async fn an_unreadable_progress_writes_nothing_and_signals_failure() {
    let unreachable = RepositoryError::Timeout("pool timed out".into());
    let (worker, repository, heartbeat) = worker(Err(unreachable), None);

    assert!(worker.run_cycle(at(12, 30)).await.is_err());

    assert!(repository.cycles.lock().unwrap().is_empty());
    assert!(signals(&heartbeat)[0].contains("pool timed out"));
}

/// Against the measure run only after a written cycle: during a run of failed cycles, the
/// closed hours would go without their TVL.
#[tokio::test]
async fn a_failed_cycle_still_measures_the_last_closed_hour() {
    let refused = RepositoryError::Backend("canceling statement due to statement timeout".into());
    let (worker, repository, _) = worker(Ok(PROGRESS), Some(refused));

    assert!(worker.run_cycle(at(12, 30)).await.is_err());

    assert_eq!(
        *repository.measures.lock().unwrap(),
        [(at(11, 0), at(12, 30))]
    );
}

/// Against the heartbeat following the cycle alone: a measure failing for good would never
/// reach the alarm.
#[tokio::test]
async fn a_failed_measure_alone_signals_its_error() {
    let refused =
        RepositoryError::Backend("permission denied for table analytic_protocol_tvl".into());
    let (worker, _, heartbeat) = measuring(Ok(PROGRESS), None, Err(refused));

    assert!(worker.run_cycle(at(12, 30)).await.is_ok());

    assert_eq!(signals(&heartbeat).len(), 1);
    assert!(
        signals(&heartbeat)[0].starts_with("failure: TVL measure failed:")
            && signals(&heartbeat)[0].contains("permission denied for table analytic_protocol_tvl"),
        "{:?}",
        signals(&heartbeat)
    );
}

/// Against an hour already measured taken for a failure: eleven cycles an hour would alarm.
#[tokio::test]
async fn an_hour_already_measured_signals_success() {
    let (worker, _, heartbeat) = measuring(Ok(PROGRESS), None, Ok(TvlMeasure::AlreadyMeasured));

    worker.run_cycle(at(12, 30)).await.unwrap();

    assert_eq!(signals(&heartbeat), ["success"]);
}
