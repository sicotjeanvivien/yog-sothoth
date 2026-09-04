//! Bootstrap utilities shared across yog-sothoth's native binaries
//! (indexer, context, signals, api) and the `yog-migrate` binary.
//!
//! This crate hosts what every binary needs at startup, and only that:
//!
//! - reading and validating environment variables (`env`)
//! - wrapping secrets so they cannot be printed (`secret`)
//! - the canonical `ConfigError` type returned by every binary's
//!   `Config::load` (`error`)
//! - one-shot runtime initialization for crates that don't pick a
//!   default (rustls), and the shared tracing subscriber (`runtime`)
//!
//! Each binary keeps its own `Config` struct describing the variables
//! it cares about — only the building blocks live here. The `Config`
//! type is intentionally not generalized: the indexer's variables and
//! the api's variables don't overlap enough to share a struct, and a
//! "common" config that contains everyone's variables is a smell.

mod env;
mod error;
mod runtime;
mod secret;

/// The guard that keeps `.expose()` on the lines that consume a secret.
///
/// It lives here rather than in each crate because the rule belongs to the
/// type, and the type lives here — one definition instead of seven restatements
/// of the same convention.
#[cfg(test)]
#[path = "exposure_tests.rs"]
mod exposure_tests;

pub use env::{
    EnvEnum, duration_var, parse_required_bool, parse_required_enum, parse_required_u32, required,
    required_secret_key, required_secret_url,
};
pub use error::ConfigError;
pub use runtime::{init_rustls, init_tracing};
pub use secret::{SecretKey, SecretUrl};
