//! What the daemon does once wired: the ports it reads the outside world
//! through, and the loops that enrich the data with them.

pub(crate) mod heartbeat;
pub(crate) mod source;
pub(crate) mod source_health;
pub(crate) mod workers;
