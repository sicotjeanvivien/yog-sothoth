//! What a run calls outside the process: `pg_dump` and `pg_restore` as
//! subprocesses, and Healthchecks.io over HTTP. The run itself, which decides
//! what to call and what each outcome signals, is [`crate::archiver`].

mod dump;
mod heartbeat;

pub(crate) use dump::{Connection, PgTools, read_tail};
pub(crate) use heartbeat::{HealthchecksHeartbeat, Heartbeat};

#[cfg(test)]
pub(crate) use heartbeat::RecordingHeartbeat;
