//! Meteora-specific services, one module per product: a service whose repository, params and
//! response are irreducibly one product's. A cross-protocol one stays at the root, even with
//! DAMM v2 as its only source today.

pub(crate) mod damm_v2;
