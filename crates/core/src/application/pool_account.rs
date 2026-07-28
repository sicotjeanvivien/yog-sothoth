//! Decoding of on-chain **pool accounts** into domain properties.
//!
//! The counterpart of [`super::extraction`]: where that module turns a
//! transaction's event bytes into domain events, this one turns an account's
//! raw bytes into the pool properties that events never carry — the token
//! mints, the base fee, the fee-split percents.
//!
//! Both are protocol-specific byte layouts, both are pure, and both belong in
//! `core` for the same reason: they are the project's knowledge of what the
//! chain means, and the crates that do I/O should not have to own it.
//!
//! # Why this is not in `amm/`
//!
//! `amm/` is *math* — spot price, price impact, fee arithmetic. This is layout
//! decoding. (The fee-blob decoders in `amm::damm_v2` predate the distinction
//! and sit on the wrong side of it; moving them is tracked separately, since it
//! would churn the indexer for no functional gain.)
//!
//! # Dispatch
//!
//! An account's `owner` **is** its program id, so [`decode_pool_account`] routes
//! on it via [`Protocol::from_program_id`] — the caller does not have to know
//! which protocol it asked for, which is what lets one RPC client serve every
//! protocol.
//!
//! # The two guards, and why they are load-bearing
//!
//! Every decoder checks the owner (here, by dispatch) **and** the account's
//! Anchor discriminator. Neither is redundant.
//!
//! Reading a foreign account at cp-amm's offsets does not produce obvious
//! garbage: at byte 168 and 200 of a DLMM `LbPair` sit `reserve_x` and
//! `reserve_y` — valid, aligned `Pubkey`s. Without the guards, `Pubkey::try_from`
//! succeeds, no error is raised, and the pool's *vault* addresses are written
//! into its *mint* columns. Silently wrong data, not a crash.

mod meteora;

use solana_pubkey::Pubkey;

use crate::domain::{PoolAccountProperties, Protocol};

/// Decode a raw pool account into its protocol's properties.
///
/// `owner` selects the layout; `data` is the account's raw bytes, already
/// base64-decoded by the transport (base64 is the RPC's encoding, not the
/// chain's — `core` stays free of it).
///
/// Returns `None` — never an error — for an account this project does not
/// decode: an unknown owner, a discriminator that does not match the protocol's
/// pool account, or data too short for the layout. None of those are failures;
/// they mean "not a pool account of ours", which the caller handles by simply
/// skipping the entry and retrying next cycle.
pub fn decode_pool_account(owner: &Pubkey, data: &[u8]) -> Option<PoolAccountProperties> {
    match Protocol::from_program_id(owner)? {
        Protocol::MeteoraDammV2 => {
            meteora::damm_v2::decode_pool_account(data).map(PoolAccountProperties::MeteoraDammV2)
        }
        // No pool-account decoder yet: these protocols are recognized (so they
        // are not reported as unknown owners) but produce nothing.
        Protocol::MeteoraDammV1 | Protocol::MeteoraDlmm => None,
    }
}

#[cfg(test)]
#[path = "pool_account_tests.rs"]
mod tests;
