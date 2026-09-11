//! Whether an endpoint's scheme is one a path can speak, asked once for both.
//!
//! # Why both paths ask
//!
//! `INGEST_STREAM_URL` is read by **both** sources, so switching
//! `INGEST_SOURCE` and forgetting the URL is the ordinary mistake. Neither
//! client refuses the other's scheme on its own: tonic accepts a `wss://` URI
//! and fails inside its retry loop, `PubsubClient` accepts an `https://` one and
//! fails per worker. Refused here, the fault is named at start-up instead of
//! arriving as an exhausted budget that reads like an unreachable provider.
//!
//! # What is shared, and what is not
//!
//! Shared: the sort into three outcomes — accepted, a foreign scheme, no scheme
//! at all — and the parser behind it, [`SecretUrl::scheme`]. The two paths used
//! to answer with two parsers, tonic's on one side and this one on the other,
//! which is one rule written twice and free to drift.
//!
//! **Not shared: the messages.** Each says what *its* path expects and which
//! *other* source reads the URL it was handed, so each listener maps a
//! [`SchemeRefusal`] to its own `InvalidEndpoint`.

use yog_bootstrap::SecretUrl;

/// Why a scheme was refused. Two variants because they are two mistakes:
/// telling an operator their endpoint "carries the `` scheme" reads like a bug
/// in the message.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SchemeRefusal {
    /// A scheme is there, and it belongs to something else — lowercased.
    Foreign(String),
    /// No scheme at all.
    Missing,
}

/// Accept `url` when its scheme is in `accepted`, which must be lowercase —
/// [`SecretUrl::scheme`] lowercases what it reads.
pub(crate) fn check(url: &SecretUrl, accepted: &[&str]) -> Result<(), SchemeRefusal> {
    match url.scheme() {
        Some(scheme) if accepted.contains(&scheme.as_str()) => Ok(()),
        Some(scheme) => Err(SchemeRefusal::Foreign(scheme)),
        None => Err(SchemeRefusal::Missing),
    }
}

#[cfg(test)]
#[path = "scheme_tests.rs"]
mod tests;
