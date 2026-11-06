//! The lines the gRPC listener writes, one function each.

use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::SubscribeRequest;
use yog_bootstrap::Endpoint;

use yog_core::{RepositoryError, domain::Protocol};

/// The configuration has been read and the first attempt is about to start.
pub(super) fn starting(endpoint: &Endpoint, header: Option<&str>, request: &SubscribeRequest) {
    info!(
        endpoint = %endpoint,
        header = header.unwrap_or("none"),
        filters = request.transactions.len(),
        accounts = request
            .transactions
            .values()
            .map(|f| f.account_include.len())
            .sum::<usize>(),
        "gRPC listener starting"
    );
}

/// The server answered `subscribe`.
pub(super) fn subscribed() {
    info!("subscribed to the Yellowstone stream");
}

pub(super) fn stopping_on_shutdown() {
    info!("shutdown requested — gRPC listener stopping");
}

pub(super) fn stopping_consumer_gone() {
    info!("downstream channel closed — gRPC listener stopping");
}

/// A stream that delivered ended: cleanly when there is no `error`, broken or
/// stalled otherwise.
pub(super) fn resubscribing(attempt: u32, error: Option<&str>) {
    match error {
        None => warn!(attempt, "gRPC stream closed — resubscribing"),
        Some(error) => warn!(attempt, error = %error, "gRPC stream broke — resubscribing"),
    }
}

/// An attempt that delivered nothing is charged to the budget.
pub(super) fn attempt_failed(attempt: u32, max: u32, error: &str) {
    warn!(attempt, max, error = %error, "gRPC stream attempt failed");
}

/// A session's slots could not be written; the next tick writes them again.
pub(super) fn slots_not_recorded(protocol: Protocol, error: &RepositoryError) {
    warn!(
        protocol = protocol.as_str(),
        %error,
        "could not write the slots received — retried at the next tick"
    );
}
