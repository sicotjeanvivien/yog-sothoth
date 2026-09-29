//! The dead man's switch: tell Healthchecks.io how a daemon's work went.
//!
//! The service raises the alarm when an expected signal does **not** arrive,
//! which is the only alert a stopped or wedged daemon can still produce. A
//! failure is signalled too, with its reason, so the alert says why.
//!
//! A signal that cannot be delivered is logged and counted, and changes
//! nothing else: the work already happened or already failed, and the missing
//! signal will itself raise the alarm on the other side.
//!
//! Behind the `heartbeat` feature: it brings an HTTP client, which only the
//! daemons that report to a check should pay for.

use std::time::Duration;

use async_trait::async_trait;
use tracing::warn;

use crate::SecretUrl;

/// Long enough for a slow network, short enough that a hung endpoint does
/// not hold the next run.
const TIMEOUT: Duration = Duration::from_secs(15);

#[async_trait]
pub trait Heartbeat: Send + Sync {
    /// The work went as it should.
    async fn success(&self);
    /// The work failed; `reason` says how.
    async fn failure(&self, reason: &str);
}

/// What differs from one daemon's check to another's — the words its errors
/// and its metrics use, never how a signal is sent.
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatSettings {
    /// The variable the URL was read from, named when the URL is refused.
    pub variable: &'static str,
    /// The counter of signals that could not be delivered, labelled by `kind`
    /// (`success` / `failure`). The daemon describes it with its own metrics.
    pub undelivered_counter: &'static str,
}

/// Healthchecks.io's ping API: `POST <check-url>` on success,
/// `POST <check-url>/fail` on failure, the body shown in its log.
///
/// `/fail` goes into the URL's **path**. Appended as text, it would land in
/// the query of a slug URL (`…/<ping-key>/<slug>?create=1/fail`), and the
/// service would record every failure as a success — the one mistake a dead
/// man's switch exists to prevent.
pub struct HealthchecksHeartbeat {
    client: reqwest::Client,
    url: SecretUrl,
    settings: HeartbeatSettings,
}

impl HealthchecksHeartbeat {
    /// Refuses, at startup, a URL that cannot take `/fail`: better a daemon
    /// that will not start than one whose failures cannot be reported. The
    /// error does not quote the URL, which carries the check's secret.
    pub fn new(url: SecretUrl, settings: HeartbeatSettings) -> anyhow::Result<Self> {
        if fail_url(url.expose()).is_none() {
            anyhow::bail!(
                "{} is not a URL a `/fail` path can be added to",
                settings.variable
            );
        }
        let client = reqwest::Client::builder().timeout(TIMEOUT).build()?;
        Ok(Self {
            client,
            url,
            settings,
        })
    }

    async fn deliver(&self, request: reqwest::RequestBuilder, kind: &'static str) {
        let result = request
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        if let Err(e) = result {
            self.undelivered(kind);
            warn!(
                kind,
                error = %self.url.scrub(&e.to_string()),
                "the heartbeat could not be delivered — the missing signal will raise the alarm on its own"
            );
        }
    }

    fn undelivered(&self, kind: &'static str) {
        metrics::counter!(self.settings.undelivered_counter, "kind" => kind).increment(1);
    }
}

#[async_trait]
impl Heartbeat for HealthchecksHeartbeat {
    async fn success(&self) {
        let request = self.client.post(self.url.expose());
        self.deliver(request, "success").await;
    }

    async fn failure(&self, reason: &str) {
        let Some(url) = fail_url(self.url.expose()) else {
            // Checked at startup; kept total rather than unwrapped.
            self.undelivered("failure");
            return;
        };
        let request = self.client.post(url).body(reason.to_string());
        self.deliver(request, "failure").await;
    }
}

/// The check's URL with `fail` pushed onto its path, query kept as is.
fn fail_url(check: &str) -> Option<url::Url> {
    let mut url = url::Url::parse(check).ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().push("fail");
    Some(url)
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;

/// A heartbeat that remembers what it was told, for the tests of the daemons
/// that signal. Behind `test-support`, like the secrets' `for_tests`.
#[cfg(feature = "test-support")]
#[derive(Default)]
pub struct RecordingHeartbeat {
    pub signals: std::sync::Mutex<Vec<String>>,
}

#[cfg(feature = "test-support")]
#[async_trait]
impl Heartbeat for RecordingHeartbeat {
    async fn success(&self) {
        self.signals.lock().unwrap().push("success".to_string());
    }

    async fn failure(&self, reason: &str) {
        self.signals
            .lock()
            .unwrap()
            .push(format!("failure: {reason}"));
    }
}
