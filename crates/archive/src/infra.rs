//! What a run calls outside the process and outside the database layer:
//! Healthchecks.io over HTTP, and the per-run read of the server's versions.
//! The dump itself is `yog_persistence`'s ([`yog_persistence::PgTools`]). The
//! run, which decides what to call and what each outcome signals, is
//! [`crate::archiver`].

mod heartbeat;
mod versions;

pub(crate) use heartbeat::{HealthchecksHeartbeat, Heartbeat};
pub(crate) use versions::PgVersions;

#[cfg(test)]
pub(crate) use heartbeat::RecordingHeartbeat;
