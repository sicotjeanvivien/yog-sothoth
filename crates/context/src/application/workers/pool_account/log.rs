//! The pool-account worker's log lines, one function each.

use std::time::Duration;

use solana_pubkey::Pubkey;
use tracing::{debug, info, warn};
use yog_core::RepositoryError;
use yog_core::application::PoolAccountRejection;
use yog_core::domain::{FieldOutOfRange, PoolAccountDegradation, Protocol};

use crate::error::SourceError;

pub(super) fn started() {
    info!("PoolAccountWorker started");
}

/// A stop was asked; the worker leaves between two cycles.
pub(super) fn stopping() {
    info!("shutdown requested — pool-account worker stopping");
}

pub(super) fn list_unresolved_failed(protocol: Protocol, error: &RepositoryError) {
    warn!(protocol = %protocol, error = %error, "pool-account worker: list_unresolved failed");
}

pub(super) fn nothing_unresolved(protocol: Protocol) {
    debug!(protocol = %protocol, "pool-account worker: no unresolved pools");
}

pub(super) fn source_failed(protocol: Protocol, error: &SourceError) {
    warn!(protocol = %protocol, error = %error, "pool-account worker: source returned a hard error");
}

pub(super) fn rejected(protocol: Protocol, pool: &Pubkey, rejection: &PoolAccountRejection) {
    warn!(
        protocol = %protocol,
        pool = %pool,
        reason = %rejection,
        "pool-account worker: account rejected",
    );
}

pub(super) fn protocol_disagrees(protocol: Protocol, decoded_as: Protocol, pool: &Pubkey) {
    warn!(
        protocol = %protocol,
        decoded_as = %decoded_as,
        pool = %pool,
        "pool-account worker: queue and account disagree on protocol — skipping",
    );
}

/// The account is written, without the field `degradation` names.
pub(super) fn degraded(protocol: Protocol, pool: &Pubkey, degradation: &PoolAccountDegradation) {
    match degradation {
        PoolAccountDegradation::BaseFeeModeUnknown => warn!(
            protocol = %protocol,
            pool = %pool,
            "pool-account worker: base fee mode unknown — base_fee_kind not updated",
        ),
        PoolAccountDegradation::AccountTooShort { len, needed } => warn!(
            protocol = %protocol,
            pool = %pool,
            len,
            needed,
            "pool-account worker: account too short — written without its optional tail",
        ),
    }
}

/// The satellite is written with an empty fee curve: `field` does not fit the store.
pub(super) fn fee_scheduler_out_of_range(protocol: Protocol, pool: &Pubkey, out: FieldOutOfRange) {
    warn!(
        protocol = %protocol,
        pool = %pool,
        field = out.field,
        "pool-account worker: fee curve out of range — written without it",
    );
}

pub(super) fn set_pool_account_failed(protocol: Protocol, pool: &Pubkey, error: &RepositoryError) {
    warn!(protocol = %protocol, pool = %pool, error = %error,
          "pool-account worker: set_pool_account failed");
}

pub(super) fn set_registry_properties_failed(
    protocol: Protocol,
    pool: &Pubkey,
    error: &RepositoryError,
) {
    warn!(protocol = %protocol, pool = %pool, error = %error,
          "pool-account worker: set_registry_properties failed");
}

pub(super) fn cycle_done(
    protocol: Protocol,
    requested: usize,
    fetched: usize,
    decoded: usize,
    rejected: usize,
    written: usize,
    elapsed: Duration,
) {
    debug!(
        protocol = %protocol,
        requested,
        fetched,
        decoded,
        rejected,
        written,
        elapsed_s = elapsed.as_secs_f64(),
        "pool-account worker: cycle done",
    );
}
