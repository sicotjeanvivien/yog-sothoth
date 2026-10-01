//! The lines the gRPC listener writes, one function each.
//!
//! The listener's files decide; this one says what they decided. Same shape as
//! `bootstrap/daemon/config_log.rs`: a call site names the event, and the
//! wording and fields live here, once.

use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::SubscribeRequest;
use yog_bootstrap::Endpoint;

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
pub(super) fn subscribed(from_slot: Option<u64>) {
    info!(from_slot = ?from_slot, "subscribed to the Yellowstone stream");
}

pub(super) fn stopping_on_shutdown() {
    info!("shutdown requested — gRPC listener stopping");
}

pub(super) fn stopping_consumer_gone() {
    info!("downstream channel closed — gRPC listener stopping");
}

/// A stream that delivered ended: cleanly when there is no `error`, broken or
/// stalled otherwise.
pub(super) fn resubscribing(attempt: u32, error: Option<&str>, resume_from: Option<u64>) {
    match error {
        None => warn!(attempt, "gRPC stream closed — resubscribing"),
        Some(error) => warn!(
            attempt,
            error = %error,
            resume_from = ?resume_from,
            "gRPC stream broke — resubscribing"
        ),
    }
}

/// An attempt that delivered nothing — refused, unreachable or silent — is
/// charged to the budget.
pub(super) fn attempt_failed(attempt: u32, max: u32, error: &str, resume_from: Option<u64>) {
    warn!(
        attempt,
        max,
        error = %error,
        resume_from = ?resume_from,
        "gRPC stream attempt failed"
    );
}
