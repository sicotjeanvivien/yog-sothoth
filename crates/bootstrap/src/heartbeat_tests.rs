use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::*;

#[test]
fn fail_goes_into_the_path_of_a_plain_check_url() {
    let url = fail_url("https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666").unwrap();
    assert_eq!(
        url.as_str(),
        "https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666/fail"
    );
}

#[test]
fn fail_goes_into_the_path_of_a_slug_url_not_its_query() {
    let url = fail_url("https://hc-ping.com/PING-KEY/yog-archive?create=1").unwrap();
    assert_eq!(url.path(), "/PING-KEY/yog-archive/fail");
    assert_eq!(url.query(), Some("create=1"));
}

#[test]
fn a_trailing_slash_does_not_make_an_empty_segment() {
    let url = fail_url("https://hc-ping.com/abc/").unwrap();
    assert_eq!(url.path(), "/abc/fail");
}

#[test]
fn a_url_that_cannot_take_a_path_is_refused() {
    assert!(fail_url("not a url").is_none());
    assert!(fail_url("mailto:ops@example.com").is_none());
}

const SECRET: &str = "0b7c5a1e-1111-2222-3333-444455556666";
const UNDELIVERED: &str = "test_heartbeat_undelivered_total";

/// Held by every test that reaches the warning of `deliver`.
/// ⚠️ A log site first reached on a thread with no subscriber, while another thread's is the only
/// one, is cached as silent: the test reading the log would find it empty.
static WARNING: Mutex<()> = Mutex::new(());

fn warning_alone() -> MutexGuard<'static, ()> {
    WARNING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One request as the local check received it.
#[derive(Debug, PartialEq)]
struct Received {
    request_line: String,
    body: String,
}

/// A local check answering `status` to one request: its URL, and what it received.
async fn check_answering(status: u16) -> (String, JoinHandle<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    let received = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("the signal arrives");
        let mut raw = Vec::new();
        let mut chunk = [0u8; 1024];
        let (head, length) = loop {
            let read = socket.read(&mut chunk).await.expect("the request reads");
            raw.extend_from_slice(&chunk[..read]);
            if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&raw[..end]).to_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map_or(0, |n| n.parse::<usize>().expect("a length"));
                break (end + 4, length);
            }
            assert_ne!(read, 0, "closed before the end of the headers");
        };
        while raw.len() < head + length {
            let read = socket.read(&mut chunk).await.expect("the body reads");
            assert_ne!(read, 0, "closed before the end of the body");
            raw.extend_from_slice(&chunk[..read]);
        }
        let response =
            format!("HTTP/1.1 {status} Scripted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        socket
            .write_all(response.as_bytes())
            .await
            .expect("the answer writes");
        let text = String::from_utf8_lossy(&raw);
        Received {
            request_line: text.lines().next().expect("a request line").to_string(),
            body: String::from_utf8_lossy(&raw[head..]).to_string(),
        }
    });
    (format!("http://127.0.0.1:{port}/{SECRET}"), received)
}

/// A URL nothing listens on: the port is released before the signal is sent.
async fn unreachable_check() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    format!("http://127.0.0.1:{port}/{SECRET}")
}

fn heartbeat(url: &str) -> HealthchecksHeartbeat {
    HealthchecksHeartbeat::new(
        SecretUrl::new(url),
        HeartbeatSettings {
            variable: "TEST_HEARTBEAT_URL",
            undelivered_counter: UNDELIVERED,
        },
    )
    .expect("the URL takes `/fail`")
}

/// Drives `body` under a thread-local recorder, on a runtime of this thread, and returns what it
/// gave with the undelivered signals counted, by kind.
fn counting<T>(body: impl Future<Output = T>) -> (T, Vec<(String, u64)>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let given = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(body)
    });
    let mut counted: Vec<(String, u64)> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, _, _, _)| key.key().name() == UNDELIVERED)
        .map(|(key, _, _, value)| {
            let kind = key
                .key()
                .labels()
                .find(|label| label.key() == "kind")
                .expect("a kind")
                .value()
                .to_string();
            match value {
                DebugValue::Counter(n) => (kind, n),
                other => panic!("expected a counter, got {other:?}"),
            }
        })
        .collect();
    counted.sort();
    (given, counted)
}

fn one(kind: &str) -> Vec<(String, u64)> {
    vec![(kind.to_string(), 1)]
}

/// Mutation: the success sent somewhere else, or a delivered signal counted as undelivered.
#[test]
fn a_success_is_a_post_on_the_check_url() {
    let (received, counted) = counting(async {
        let (url, received) = check_answering(200).await;
        heartbeat(&url).report_success().await;
        received.await.expect("the local check")
    });

    assert_eq!(
        received,
        Received {
            request_line: format!("POST /{SECRET} HTTP/1.1"),
            body: String::new(),
        }
    );
    assert!(counted.is_empty(), "{counted:?}");
}

/// Mutation: the failure sent on the check's URL, which records it as a success; `/fail` appended
/// as text, which lands in the query; or the reason dropped from the body.
#[test]
fn a_failure_is_a_post_on_its_fail_path_with_the_reason() {
    let (received, counted) = counting(async {
        let (url, received) = check_answering(200).await;
        heartbeat(&format!("{url}?create=1"))
            .report_failure("the aggregate is late")
            .await;
        received.await.expect("the local check")
    });

    assert_eq!(
        received,
        Received {
            request_line: format!("POST /{SECRET}/fail?create=1 HTTP/1.1"),
            body: "the aggregate is late".to_string(),
        }
    );
    assert!(counted.is_empty(), "{counted:?}");
}

/// Mutation: `error_for_status` removed, or a kind counted under the other.
#[test]
fn a_signal_the_check_refuses_is_counted_under_its_kind() {
    let _alone = warning_alone();
    let (_, counted) = counting(async {
        let (url, received) = check_answering(500).await;
        heartbeat(&url).report_success().await;
        received.await.expect("the local check")
    });
    assert_eq!(counted, one("success"));

    let (_, counted) = counting(async {
        let (url, received) = check_answering(500).await;
        heartbeat(&url).report_failure("late").await;
        received.await.expect("the local check")
    });
    assert_eq!(counted, one("failure"));
}

/// Mutation: an undelivered signal no longer counted, or a kind counted under the other.
#[test]
fn a_signal_that_finds_no_check_is_counted_under_its_kind() {
    let _alone = warning_alone();
    let (_, counted) = counting(async {
        heartbeat(&unreachable_check().await).report_success().await;
    });
    assert_eq!(counted, one("success"));

    let (_, counted) = counting(async {
        heartbeat(&unreachable_check().await)
            .report_failure("late")
            .await;
    });
    assert_eq!(counted, one("failure"));
}

/// Mutation: the URL quoted in the refusal, which would log the check's secret at startup.
#[test]
fn a_url_that_cannot_take_fail_is_refused_without_being_quoted() {
    let refused = HealthchecksHeartbeat::new(
        SecretUrl::new(format!("mailto:{SECRET}@example.com")),
        HeartbeatSettings {
            variable: "TEST_HEARTBEAT_URL",
            undelivered_counter: UNDELIVERED,
        },
    )
    .err()
    .expect("a refusal")
    .to_string();

    assert!(refused.contains("TEST_HEARTBEAT_URL"), "{refused}");
    assert!(!refused.contains(SECRET), "{refused}");
}

/// Everything logged while `body` runs on this thread.
#[derive(Clone, Default)]
struct Logged(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Logged {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("the log").extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Mutation: `scrub` removed from the warning; `reqwest` quotes the URL, and the URL is the secret.
#[test]
fn an_undelivered_signal_keeps_the_secret_out_of_the_log() {
    let _alone = warning_alone();
    let logged = Logged::default();
    let writer = logged.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        counting(async {
            let (url, received) = check_answering(500).await;
            heartbeat(&url).report_failure("late").await;
            received.await.expect("the local check");
        })
    });

    let text = String::from_utf8_lossy(&logged.0.lock().expect("the log")).to_string();
    assert!(text.contains("could not be delivered"), "{text}");
    assert!(!text.contains(SECRET), "{text}");
}
