//! Whether an endpoint's scheme is one a path can speak, asked once for both.
//!
//! # Why both paths ask
//!
//! `INGEST_STREAM_URL` is read by **both** sources, so switching
//! `INGEST_SOURCE` and forgetting the URL is the ordinary mistake. Neither
//! client refuses the other's scheme on its own, and each fails somewhere worse
//! than start-up — see the two [`Transport`]s below. Refused here, the fault is
//! named before anything is dialled, instead of arriving as an exhausted retry
//! budget that reads like an unreachable provider.
//!
//! # What differs between the two paths is text
//!
//! The check, the parser ([`SecretUrl::scheme`]) and the shape of the refusal
//! are one; what each path says — which schemes it takes, what to write, which
//! other source reads the URL it was handed — is a [`Transport`]. The two
//! descriptions sit side by side on purpose: each names the other, and whoever
//! edits one should be looking at both.

use yog_bootstrap::SecretUrl;

/// What one ingestion path accepts, and what it tells an operator who got it
/// wrong.
pub(crate) struct Transport {
    /// Lowercase — [`SecretUrl::scheme`] lowercases what it reads.
    accepted: &'static [&'static str],
    /// What to write instead.
    expects: &'static str,
    /// What the other path's URL looks like, and which source reads it.
    other: &'static str,
}

/// The JSON-RPC path's `logsSubscribe` socket.
///
/// ⚠️ **`PubsubClient::new` accepts an `https://` URL** and just fails to
/// connect — so without this refusal the failure arrives as
/// `RPC_WORKER_MAX_RETRIES` attempts with backoff **per watched pool**, then
/// `AllWorkersGaveUp`: a configuration fault multiplied by the fleet.
///
/// Two schemes and not one: `ws://` is what a local validator speaks, and
/// narrowing this to `wss` would break every local setup while every remote one
/// stayed green.
pub(crate) const WEBSOCKET: Transport = Transport {
    accepted: &["ws", "wss"],
    expects: "`logsSubscribe` is a WebSocket: write `wss://host`, or `ws://` for a local validator.",
    other: "An `https://` address is what a Yellowstone gRPC endpoint looks like — `INGEST_SOURCE=grpc` is what reads it.",
};

/// The Yellowstone stream.
///
/// ⚠️ **tonic's `Endpoint::from_shared` accepts a `wss://` URL** — it is a
/// valid URI — and fails inside the retry loop, on an h2 handshake against
/// something that speaks WebSocket. The likeliest misconfiguration of the two:
/// `.env.example` ships a `wss://` `INGEST_STREAM_URL`, and an operator flipping
/// only `INGEST_SOURCE=grpc` keeps it.
pub(crate) const GRPC: Transport = Transport {
    accepted: &["http", "https"],
    expects: "Yellowstone speaks HTTP/2: write `https://host:port`, or `http://` for a self-hosted plaintext endpoint.",
    other: "A `wss://` address is the WebSocket endpoint of the JSON-RPC path — `INGEST_SOURCE=rpc` is what reads it.",
};

/// Accept `url` when `transport` speaks its scheme; otherwise say why not.
///
/// Two refusals, because they are two mistakes: a scheme that belongs to
/// something else, and no scheme at all — telling an operator their endpoint
/// "carries the `` scheme" reads like a bug in the message.
pub(crate) fn check(url: &SecretUrl, transport: &Transport) -> Result<(), String> {
    match url.scheme() {
        Some(scheme) if transport.accepted.contains(&scheme.as_str()) => Ok(()),
        Some(scheme) => Err(format!(
            "it carries the `{scheme}` scheme. {} {}",
            transport.expects, transport.other
        )),
        None => Err(format!("it has no scheme. {}", transport.expects)),
    }
}

#[cfg(test)]
#[path = "scheme_tests.rs"]
mod tests;
