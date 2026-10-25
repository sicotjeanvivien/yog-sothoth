//! The pool catalog's log lines, one function each.

use tracing::warn;

/// A side's value or a TVL beyond what a `Decimal` holds, served unvalued.
pub(super) fn value_out_of_range(pool_address: &str, field: &str, value: &str) {
    warn!(
        pool_address,
        field, value, "pool catalog: value beyond a Decimal, served unvalued",
    );
}
