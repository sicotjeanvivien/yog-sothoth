//! The lines the key ring writes, one function each.

use tracing::warn;
use yog_core::RepositoryError;

pub(super) fn not_reloaded(error: &RepositoryError) {
    warn!(%error, "api keys not reloaded — the ring keeps the keys it held");
}
