//! The daemon's log lines, one function each.

use std::num::NonZeroU32;
use std::time::Duration;

use tracing::info;
use yog_bootstrap::Endpoint;

/// The database is connected.
pub(super) fn connected_to_database() {
    info!("connected to database");
}

/// The database is ready for the repositories.
pub(super) fn database_initialized() {
    info!("database initialized");
}

/// The Jupiter client's spacing, which an operator cannot read off the
/// configuration.
pub(super) fn jupiter_spacing(rate_limit: NonZeroU32, request_every: Duration) {
    info!(
        rate_limit_per_minute = rate_limit.get(),
        request_every = ?request_every,
        "Jupiter requests spaced under the key's rate limit"
    );
}

/// The two Solana endpoints.
pub(super) fn endpoints_initialized(token_metadata: &Endpoint, pool_account: &Endpoint) {
    info!(
        token_metadata = %token_metadata,
        pool_account = %pool_account,
        "external endpoints initialized"
    );
}

/// The stop was asked for; the workers are told and waited on.
pub(super) fn cancellation_received() {
    info!("cancellation received — stopping");
}
