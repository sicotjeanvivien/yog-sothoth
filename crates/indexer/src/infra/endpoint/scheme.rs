//! Whether an endpoint's scheme is one a path can speak, asked once for both: `INGEST_STREAM_URL`
//! is read by both sources, and neither client refuses the other's scheme by itself.

use yog_bootstrap::SecretUrl;

/// What one ingestion path accepts, and what it tells an operator who got it wrong.
pub(crate) struct Transport {
    /// Lowercase — [`SecretUrl::scheme`] lowercases what it reads.
    accepted: &'static [&'static str],
    /// What to write instead.
    expects: &'static str,
    /// What the other path's URL looks like, and which source reads it.
    other: &'static str,
}

/// The JSON-RPC path's `logsSubscribe` socket; `ws://` too, what a local validator speaks.
/// ⚠️ `PubsubClient::new` accepts `https://` and just fails to connect, once per watched pool.
pub(crate) const WEBSOCKET: Transport = Transport {
    accepted: &["ws", "wss"],
    expects: "`logsSubscribe` is a WebSocket: write `wss://host`, or `ws://` for a local validator.",
    other: "An `https://` address is what a Yellowstone gRPC endpoint looks like — `INGEST_SOURCE=grpc` is what reads it.",
};

/// The Yellowstone stream.
/// ⚠️ tonic's `Endpoint::from_shared` accepts `wss://` and fails in the retry loop; `.env.example`
/// ships a `wss://` URL, so flipping only `INGEST_SOURCE=grpc` keeps it.
pub(crate) const GRPC: Transport = Transport {
    accepted: &["http", "https"],
    expects: "Yellowstone speaks HTTP/2: write `https://host:port`, or `http://` for a self-hosted plaintext endpoint.",
    other: "A `wss://` address is the WebSocket endpoint of the JSON-RPC path — `INGEST_SOURCE=rpc` is what reads it.",
};

/// Accept `url` when `transport` speaks its scheme; a foreign scheme and no scheme at all are
/// refused with two different messages.
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
