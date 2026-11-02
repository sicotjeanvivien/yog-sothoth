//! The lines the coverage writer writes, one function each.

use tracing::warn;
use yog_core::{RepositoryError, domain::Protocol};

/// A range could not be recorded; the next tick writes it again, wider.
pub(super) fn write_failed(protocol: Protocol, error: &RepositoryError) {
    warn!(
        protocol = protocol.as_str(),
        %error,
        "could not record the slots observed — retried at the next tick"
    );
}
