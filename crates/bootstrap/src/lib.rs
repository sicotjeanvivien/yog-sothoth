//! What every binary needs at startup, and only that: environment variables,
//! secrets and endpoints, `ConfigError`, rustls and tracing, the stop, and the
//! dead man's switch. Each binary keeps its own `Config`.

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
