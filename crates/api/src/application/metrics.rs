//! The API's Prometheus series, served on `:9000` like the daemons'.

use metrics::{counter, describe_counter};

use crate::application::{CredentialKind, ReloadOutcome, Unauthorized};

/// Requests that reached a route, by `client` (`anonymous` without a key) and route template.
const REQUESTS: &str = "yog_api_requests_total";

/// Requests refused for their credential, by [`CredentialKind`] and [`Unauthorized`] reason.
const UNAUTHORIZED: &str = "yog_api_unauthorized_total";

/// Tickets issued, by `client`.
const TICKETS_ISSUED: &str = "yog_api_tickets_issued_total";

/// Rereads of the key ring, by [`ReloadOutcome`].
const KEY_RING_RELOADS: &str = "yog_api_key_ring_reloads_total";

/// The `client` label of a request that presented no key.
pub(crate) const ANONYMOUS: &str = "anonymous";

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
            "Requests refused for their credential, labelled by credential (key or \
             ticket) and reason: missing, scheme, malformed, unknown or expired"
        );
        describe_counter!(TICKETS_ISSUED, "Tickets issued, labelled by client");
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

    pub(crate) fn record_unauthorized(credential: CredentialKind, reason: Unauthorized) {
        counter!(UNAUTHORIZED, "credential" => credential.as_str(), "reason" => reason.as_str())
            .increment(1);
    }

    pub(crate) fn record_ticket_issued(client: &str) {
        counter!(TICKETS_ISSUED, "client" => client.to_string()).increment(1);
    }

    pub(crate) fn record_key_ring_reload(outcome: ReloadOutcome) {
        counter!(KEY_RING_RELOADS, "outcome" => outcome.as_str()).increment(1);
    }
}
