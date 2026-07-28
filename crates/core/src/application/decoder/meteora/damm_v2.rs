//! Decoding of the cp-amm (Meteora DAMM v2) `Pool` account.
//!
//! # Layout
//!
//! An 8-byte Anchor discriminator, then fixed-offset fields. Empirically
//! verified against mainnet and stable across the program's ABI:
//!
//! - `cliff_fee_numerator` (base fee) — the leading `u64` at byte offset 8
//!   (`pool_fees` is the first field; its base fee numerator leads it)
//! - `protocol_fee_percent` (`u8`) at offset 48 — right after the 40-byte
//!   `BaseFeeStruct` — then `partner_fee_percent` (49) and
//!   `referral_fee_percent` (50)
//! - `token_a_mint` at offset 168 (32 bytes)
//! - `token_b_mint` at offset 200 (32 bytes)
//!
//! The account is 1112 bytes long.
//!
//! # Why the account and not the events
//!
//! These properties cannot be resolved from the transaction stream. The mints
//! were previously inferred from a per-event `transferChecked` heuristic, which
//! mis-resolved routed and multi-hop transactions; the base fee is only emitted
//! at pool genesis (`InitializePool`), which the indexer never sees for a pool
//! created before it started watching. Reading the account back-fills both for
//! every pool, old or new.

use solana_pubkey::Pubkey;

use crate::amm::damm_v2::fee_numerator_to_bps;
use crate::application::decoder::PoolAccountRejection;
use crate::domain::{MeteoraDammV2PoolAccountProperties, Protocol};

/// Anchor account discriminator for the cp-amm `Pool` account
/// (`sha256("account:Pool")[..8]`).
///
/// Checked on every decode, and **not** redundant with the owner dispatch: it
/// is the guard against decoding a *different* account of the same program at
/// this layout.
pub(in crate::application::decoder) const POOL_DISCRIMINATOR: [u8; 8] =
    [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];

/// `cliff_fee_numerator`: the leading `u64` of `pool_fees`, right after the
/// 8-byte discriminator. The same quantity decoded from the genesis event.
pub(in crate::application::decoder) const CLIFF_FEE_NUMERATOR_OFFSET: usize = 8;
/// Fee-split percents (`u8` each), immediately after the 40-byte `BaseFeeStruct`
/// inside `PoolFeesStruct`.
pub(in crate::application::decoder) const PROTOCOL_FEE_PERCENT_OFFSET: usize = 48;
pub(in crate::application::decoder) const PARTNER_FEE_PERCENT_OFFSET: usize = 49;
pub(in crate::application::decoder) const REFERRAL_FEE_PERCENT_OFFSET: usize = 50;
pub(in crate::application::decoder) const TOKEN_A_MINT_OFFSET: usize = 168;
pub(in crate::application::decoder) const TOKEN_B_MINT_OFFSET: usize = 200;

/// Minimum length for every field above to be in bounds.
const MIN_LEN: usize = TOKEN_B_MINT_OFFSET + 32;

/// Decode a cp-amm `Pool` account.
///
/// The caller has already routed on the program id, so this carries the second
/// of the two guards described in [`super::super`] — the discriminator — and
/// distinguishes it from a truncated account, because the two mean very
/// different things: a wrong discriminator is the wrong account, a short one is
/// most likely an ABI change.
pub(in crate::application::decoder) fn decode_pool_account(
    data: &[u8],
) -> Result<MeteoraDammV2PoolAccountProperties, PoolAccountRejection> {
    const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

    if data.len() < MIN_LEN {
        return Err(PoolAccountRejection::Truncated {
            protocol: PROTOCOL,
            len: data.len(),
            min: MIN_LEN,
        });
    }
    if data[..8] != POOL_DISCRIMINATOR {
        return Err(PoolAccountRejection::NotAPoolAccount { protocol: PROTOCOL });
    }

    // Every slice below is in bounds: the length check above covers the whole
    // layout, so these conversions cannot fail.
    let cliff_fee_numerator = u64::from_le_bytes(
        data[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
            .try_into()
            .expect("8 bytes, length checked above"),
    );

    Ok(MeteoraDammV2PoolAccountProperties {
        token_a_mint: Pubkey::try_from(&data[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32])
            .expect("32 bytes, length checked above"),
        token_b_mint: Pubkey::try_from(&data[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32])
            .expect("32 bytes, length checked above"),
        fee_bps: fee_numerator_to_bps(cliff_fee_numerator),
        protocol_fee_percent: data[PROTOCOL_FEE_PERCENT_OFFSET],
        partner_fee_percent: data[PARTNER_FEE_PERCENT_OFFSET],
        referral_fee_percent: data[REFERRAL_FEE_PERCENT_OFFSET],
    })
}
