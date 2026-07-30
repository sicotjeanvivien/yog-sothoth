// NOTE: no repository trait lives here, by design.
//
// The satellite is written only by yog-context, through the generic
// `PoolAccountResolver`, and read only through the generic
// `PoolPropertiesLookup`. Neither names this protocol, so neither belongs in a
// per-protocol module — the shape cp-amm converged on after #82/#83.

mod model;

pub use model::{MeteoraDlmmPoolAccountProperties, MeteoraDlmmPoolProperties};
