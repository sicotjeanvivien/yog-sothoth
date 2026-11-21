//! The reporter's rhythm: a ping at start, then one per window, each carrying that window's verdict.

use yog_bootstrap::RecordingHeartbeat;

use super::*;
use crate::application::source_health::SourceRole;

fn signals(heartbeat: &RecordingHeartbeat) -> Vec<String> {
    heartbeat.signals.lock().unwrap().clone()
}

/// Lets the spawned reporter run every step that is ready, the clock paused.
async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn a_ping_at_start_then_one_per_window_with_its_verdict() {
    let health = Arc::new(SourceHealth::default());
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let shutdown = CancellationToken::new();
    let reporter = HeartbeatReporter::new(Arc::clone(&health), heartbeat.clone());
    let task = tokio::spawn(reporter.run(shutdown.clone()));

    // Mutation: the first tick skipped — the check stays grey until the first window ends.
    settle().await;
    assert_eq!(signals(&heartbeat), ["success"]);

    health.record_failure(SourceRole::Metadata, "source HTTP error: 503".to_string());
    tokio::time::advance(HEARTBEAT_WINDOW).await;
    settle().await;
    assert_eq!(
        signals(&heartbeat),
        [
            "success",
            "failure: metadata: 1 calls, none succeeded, last error: source HTTP error: 503",
        ]
    );

    shutdown.cancel();
    task.await.expect("no panic").expect("a stop is no failure");
}

#[tokio::test(start_paused = true)]
async fn nothing_is_signalled_once_the_stop_is_asked() {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let shutdown = CancellationToken::new();
    shutdown.cancel();
    let reporter = HeartbeatReporter::new(Arc::new(SourceHealth::default()), heartbeat.clone());

    reporter.run(shutdown).await.expect("a stop is no failure");

    assert!(signals(&heartbeat).is_empty());
}
