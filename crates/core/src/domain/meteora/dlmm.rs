//! Meteora DLMM (Liquidity Book) domain types.
//!
//! Only the pool-properties satellite for now: the per-event types this module
//! will eventually hold — swaps, liquidity, bin state — belong to the DLMM
//! event chantier and are not needed to enrich a pool from its account.

mod pool_properties;

pub use pool_properties::{MeteoraDlmmPoolAccountProperties, MeteoraDlmmPoolProperties};
