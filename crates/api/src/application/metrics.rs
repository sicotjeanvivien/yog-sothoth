//! The API's Prometheus series, served on `:9000` like the daemons'.

use metrics::{counter, describe_counter};

/// Requests that reached a route, by `client` (`anonymous` without a key) and route template.
const REQUESTS: &str = "yog_api_requests_total";

/// Requests refused for their credential, by [`Unauthorized`] reason.
const UNAUTHORIZED: &str = "yog_api_unauthorized_total";

/// Rereads of the key ring, by [`KeyRingReload`] outcome.
const KEY_RING_RELOADS: &str = "yog_api_key_ring_reloads_total";

/// The `client` label of a request that presented no key.
pub(crate) const ANONYMOUS: &str = "anonymous";

/// Why a credential was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unauthorized {
    /// `Authorization` names another scheme than `Bearer`, or is not text.
    Scheme,
    /// The bearer is not an API key.
    Malformed,
    /// An API key the ring does not hold: never issued, or revoked.
    Unknown,
}

impl Unauthorized {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scheme => "scheme",
            Self::Malformed => "malformed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyRingReload {
    Reloaded,
    Failed,
}

impl KeyRingReload {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reloaded => "reloaded",
            Self::Failed => "failed",
        }
    }
}

pub(crate) struct ApiMetrics;

impl ApiMetrics {
    /// Call once at startup, after the exporter is installed.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            REQUESTS,
            "Requests that reached a route, labelled by client (anonymous without \
             a key) and route template"
        );
        describe_counter!(
            UNAUTHORIZED,
            "Requests refused for their credential, labelled by reason: scheme, \
             malformed or unknown"
        );
        describe_counter!(
            KEY_RING_RELOADS,
            "Rereads of the API keys, labelled by outcome; a failed one keeps the \
             keys held before"
        );
    }

    pub(crate) fn record_request(client: &str, route: &str) {
        counter!(REQUESTS, "client" => client.to_string(), "route" => route.to_string())
            .increment(1);
    }

    pub(crate) fn record_unauthorized(reason: Unauthorized) {
        counter!(UNAUTHORIZED, "reason" => reason.as_str()).increment(1);
    }

    pub(crate) fn record_key_ring_reload(outcome: KeyRingReload) {
        counter!(KEY_RING_RELOADS, "outcome" => outcome.as_str()).increment(1);
    }
}
