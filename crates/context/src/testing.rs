//! A local HTTP server for the provider clients' tests: it plays scripted answers in order and
//! keeps each JSON request it received; and the counters a call records.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use yog_bootstrap::SecretUrl;

pub(crate) struct ScriptedProvider {
    pub(crate) url: SecretUrl,
    received: Arc<Mutex<Vec<Value>>>,
}

impl ScriptedProvider {
    /// Answers each request with the next `(status, body)`; once they run out, connections are refused.
    pub(crate) async fn answering(script: Vec<(u16, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a local port");
        let port = listener.local_addr().expect("its address").port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&received);
        tokio::spawn(async move {
            for (status, body) in script {
                let accepted = tokio::time::timeout(Duration::from_secs(10), listener.accept());
                let Ok(Ok((mut socket, _))) = accepted.await else {
                    return;
                };
                let request = read_json_request(&mut socket).await;
                kept.lock().expect("the requests").push(request);
                let body = body.to_string();
                let response = format!(
                    "HTTP/1.1 {status} Scripted\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("the answer writes");
            }
        });
        Self {
            url: SecretUrl::for_tests(format!("http://127.0.0.1:{port}/?api-key=secret")),
            received,
        }
    }

    pub(crate) fn requests(&self) -> Vec<Value> {
        self.received.lock().expect("the requests").clone()
    }
}

async fn read_json_request(socket: &mut TcpStream) -> Value {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = socket.read(&mut chunk).await.expect("the request reads");
        assert_ne!(read, 0, "closed before the end of the request");
        raw.extend_from_slice(&chunk[..read]);
        let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&raw[..end]).to_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .expect("a JSON body has a length")
            .parse()
            .expect("a length");
        if raw.len() >= end + 4 + length {
            return serde_json::from_slice(&raw[end + 4..end + 4 + length]).expect("JSON");
        }
    }
}

/// One counter recorded: its name, its labels, its count.
pub(crate) type RecordedCounter = (String, Vec<(String, String)>, u64);

/// Run `future` on a current-thread runtime under a thread-local recorder: its output, and every
/// counter it recorded.
/// ⚠️ Not from a `#[tokio::test]`: `with_local_recorder` binds the current thread.
pub(crate) fn recording_metrics<T>(
    future: impl std::future::Future<Output = T>,
) -> (T, Vec<RecordedCounter>) {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let output = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a current-thread runtime")
            .block_on(future)
    });
    let counters = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(key, _, _, value)| {
            let metrics_util::debugging::DebugValue::Counter(count) = value else {
                return None;
            };
            let labels = key
                .key()
                .labels()
                .map(|label| (label.key().to_string(), label.value().to_string()))
                .collect();
            Some((key.key().name().to_string(), labels, count))
        })
        .collect();
    (output, counters)
}
