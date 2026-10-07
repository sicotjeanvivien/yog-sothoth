//! Bootstrap utilities shared by yog-sothoth's binaries, `yog-migrate`
//! included — what every binary needs at startup, and only that:
//!
//! - reading and validating environment variables (`env`);
//! - wrapping secrets so they cannot be printed (`secret`), and holding an
//!   endpoint's address apart from its credential (`endpoint`);
//! - the `ConfigError` every `Config::load` returns (`error`);
//! - rustls and the shared tracing subscriber (`runtime`);
//! - the stop: its signals, what a task's end says, and the grace (`shutdown`);
//! - what a daemon tells a dead man's switch (`heartbeat`, behind its feature).
//!
//! Each binary keeps its own `Config`: only the building blocks live here.

mod endpoint;
mod env;
mod error;
#[cfg(feature = "heartbeat")]
mod heartbeat;
mod runtime;
mod secret;
mod shutdown;

/// The guard that keeps `.expose()` on the lines that consume a secret. Here,
/// because the rule belongs to the type.
#[cfg(test)]
#[path = "exposure_tests.rs"]
mod exposure_tests;

pub use endpoint::Endpoint;
pub use env::{
    EnvEnum, duration_var, optional, optional_secret_url, parse_optional, parse_required_bool,
    parse_required_enum, parse_required_u32, required, required_endpoint,
    required_endpoint_allowing_header, required_secret_key, required_secret_url,
};
pub use error::ConfigError;
#[cfg(all(feature = "heartbeat", feature = "test-support"))]
pub use heartbeat::RecordingHeartbeat;
#[cfg(feature = "heartbeat")]
pub use heartbeat::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings};
pub use runtime::{init_rustls, init_tracing};
pub use secret::{SecretKey, SecretUrl};
pub use shutdown::{SHUTDOWN_GRACE, Stop, TaskEnd, handle_task_result, shutdown_signal};
