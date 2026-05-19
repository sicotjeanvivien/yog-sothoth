//! Bootstrap utilities shared across yog-sothoth's native binaries
//! (indexer, api).
//!
//! This crate hosts what every binary needs at startup, and only that:
//!
//! - reading and validating environment variables (`env`)
//! - wrapping connection strings that contain secrets (`secret`)
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

pub use env::{duration_var, parse_required_bool, parse_required_u32, required};
pub use error::ConfigError;
pub use runtime::{init_rustls, init_tracing};
pub use secret::SecretUrl;
