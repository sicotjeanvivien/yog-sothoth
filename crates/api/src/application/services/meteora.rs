//! Meteora-specific application services, one module per product.
//!
//! Mirrors `yog_core::domain::meteora`: the protocol-shaped services live under
//! their protocol's path rather than carrying it in a file name, so that
//! "general service" and "one protocol's service" are told apart by *where* a
//! file sits, not by reading its prefix.
//!
//! What belongs here: a service whose repository, query params and response are
//! irreducibly one product's (paginated swap and liquidity event feeds — their
//! columns exist only in `meteora_damm_v2_*` tables). What does not: anything
//! cross-protocol, even if DAMM v2 is its only source of data today. `PoolService`
//! stays at the root for that reason — it serves every protocol's pools and
//! reaches per-protocol data through `PoolPropertiesLookup`, naming none.

pub(crate) mod damm_v2;
