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
};

/// Answers `progress` and `write_cycle` as scripted, and keeps the cycles it
/// was asked to write.
struct ScriptedRepository {
    progress: RepositoryResult<AnalyticProgress>,
    written: RepositoryResult<CycleOutcome>,
    cycles: Mutex<Vec<CycleWrite>>,
}

#[async_trait]
impl AnalyticRepository for ScriptedRepository {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress> {
        self.progress.clone()
    }

    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome> {
        self.cycles.lock().unwrap().push(*cycle);
        self.written.clone()
    }
}

fn worker(
    progress: RepositoryResult<AnalyticProgress>,
    written: RepositoryResult<CycleOutcome>,
) -> (
    AnalyticWorker,
    Arc<ScriptedRepository>,
    Arc<RecordingHeartbeat>,
) {
    let repository = Arc::new(ScriptedRepository {
        progress,
        written,
        cycles: Mutex::new(Vec::new()),
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
    };
    let (worker, repository, heartbeat) = worker(Ok(progress), Ok(CycleOutcome::default()));

    worker.cycle(at(12, 30)).await.unwrap();

    assert_eq!(
        *repository.cycles.lock().unwrap(),
        [CycleWrite::plan(at(12, 30), progress)]
    );
    assert_eq!(signals(&heartbeat), ["success"]);
}

#[tokio::test]
async fn a_failed_write_signals_its_error() {
    let refused = RepositoryError::Backend("permission denied for table signals".into());
    let (worker, _, heartbeat) = worker(Ok(PROGRESS), Err(refused));

    assert!(worker.cycle(at(12, 30)).await.is_err());

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
    let (worker, repository, heartbeat) = worker(Err(unreachable), Ok(CycleOutcome::default()));

    assert!(worker.cycle(at(12, 30)).await.is_err());

    assert!(repository.cycles.lock().unwrap().is_empty());
    assert!(signals(&heartbeat)[0].contains("pool timed out"));
}
