//! Decoding of the Meteora DLMM (Liquidity Book) `LbPair` account.
//!
//! # Layout
//!
//! An 8-byte Anchor discriminator, then a zero-copy struct at fixed offsets.
//! Derived from lb_clmm's own `StaticParameters` / `VariableParameters` /
//! `LbPair`, and **confirmed against a live mainnet account** rather than
//! guessed from samples:
//!
//! ```text
//! 8    parameters : StaticParameters (32 bytes, 8..40)
//!      8    base_factor                 u16
//!      10   filter_period               u16
//!      12   decay_period                u16
//!      14   reduction_factor            u16
//!      16   variable_fee_control        u32
//!      20   max_volatility_accumulator  u32
//!      24   min_bin_id                  i32
//!      28   max_bin_id                  i32
//!      32   protocol_share              u16
//!      34   base_fee_power_factor       u8
//! 40   v_parameters : VariableParameters (32 bytes, 40..72)
//! 72   bump_seed / bin_step_seed / pair_type      (4 bytes)
//! 76   active_id     i32
//! 80   bin_step      u16
//! 82   status        u8
//! 88   token_x_mint  Pubkey
//! 120  token_y_mint  Pubkey
//! 152  reserve_x     Pubkey
//! 184  reserve_y     Pubkey
//! ```
//!
//! The account is 904 bytes long.
//!
//! # The offsets are verified, not inferred
//!
//! Decoded from `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` (SOL/USDC) before
//! this file existed: `base_factor = 10000`, `bin_step = 1`,
//! `base_fee_power_factor = 0`, mints `So111…112` / `EPjFW…Dt1v` — and
//! [`crate::amm::dlmm::base_fee_bps`] turns those into 1 bps, which is what
//! Meteora displays for that pool. The layout above reproduces the three offsets
//! (80 / 88 / 120) that were already independently known, which is what
//! validates the reconstruction of the ones that were not.
//!
//! # What is deliberately not decoded
//!
//! `active_id` and the whole of `VariableParameters` (the volatility
//! accumulator, its reference and decay) are **state**, not configuration: they
//! move on every swap. They belong to `pool_current_state`, not to a satellite
//! that would otherwise be rewritten on every crossed bin. Same for the
//! `filter_period` / `decay_period` / `reduction_factor` triple, which is only
//! useful to a build that recomputes the variable fee — which Yog does not.
//!
//! # Why the account and not the events
//!
//! Same reason as cp-amm: a pool discovered from the transaction stream is one
//! whose genesis we missed, so its mints and its fee parameters are only
//! available by reading the account back.

use solana_pubkey::Pubkey;

use crate::amm::dlmm::base_fee_bps;
use crate::application::decoder::PoolAccountRejection;
use crate::domain::{
    DecodedPoolAccount, MeteoraDlmmPoolAccountProperties, PoolAccountProperties,
    PoolRegistryProperties, Protocol,
};

/// Anchor account discriminator for the DLMM `LbPair` account
/// (`sha256("account:LbPair")[..8]`).
///
/// Checked on every decode, and **not** redundant with the owner dispatch: it is
/// the guard against decoding a *different* account of the same program — and
/// lb_clmm has several (`Bin Array`, `Position`, `Oracle`) — at this layout.
pub(in crate::application::decoder) const LB_PAIR_DISCRIMINATOR: [u8; 8] =
    [0x21, 0x0b, 0x31, 0x62, 0xb5, 0x65, 0xb1, 0x0d];

/// `StaticParameters` opens the account, right after the discriminator, so
/// `base_factor` is its leading `u16`.
pub(in crate::application::decoder) const BASE_FACTOR_OFFSET: usize = 8;
/// Dynamic-fee magnitude and its per-pool ceiling, at bytes 8 and 12 of
/// `StaticParameters`. `variable_fee_control == 0` means no variable fee — DLMM
/// has no boolean flag for it, unlike cp-amm's `DynamicFeeStruct::initialized`.
pub(in crate::application::decoder) const VARIABLE_FEE_CONTROL_OFFSET: usize = 16;
pub(in crate::application::decoder) const MAX_VOLATILITY_ACCUMULATOR_OFFSET: usize = 20;
/// Meteora's cut, in **basis points** — not the whole percent cp-amm uses.
pub(in crate::application::decoder) const PROTOCOL_SHARE_OFFSET: usize = 32;
/// Power-of-ten scaling on the base fee, the last byte of `StaticParameters`
/// before its padding.
pub(in crate::application::decoder) const BASE_FEE_POWER_FACTOR_OFFSET: usize = 34;
/// After both parameter blocks and the seed/type bytes, past `active_id`.
pub(in crate::application::decoder) const BIN_STEP_OFFSET: usize = 80;
pub(in crate::application::decoder) const TOKEN_X_MINT_OFFSET: usize = 88;
pub(in crate::application::decoder) const TOKEN_Y_MINT_OFFSET: usize = 120;

/// Minimum length for every field above to be in bounds.
const MIN_LEN: usize = TOKEN_Y_MINT_OFFSET + 32;

/// Read a little-endian `u16` at `offset`. The caller has already length-checked
/// the whole layout, so the slice and the conversion cannot fail.
fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        data[offset..offset + 2]
            .try_into()
            .expect("2 bytes, length checked by the caller"),
    )
}

/// Read a little-endian `u32` at `offset`. Same contract as [`u16_at`].
fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("4 bytes, length checked by the caller"),
    )
}

/// Decode a DLMM `LbPair` account.
///
/// The caller has already routed on the program id, so this carries the second
/// of the two guards described in [`super::super`] — the discriminator — and
/// distinguishes it from a truncated account, because the two mean very
/// different things: a wrong discriminator is the wrong account, a short one is
/// most likely an ABI change.
///
/// Returns the read split by **who stores it**: the neutral token pair and base
/// fee, which the `pools` registry owns, and the DLMM-only properties, which
/// this protocol's satellite owns. One read of one buffer, two writers.
///
/// # Total, unlike cp-amm's
///
/// Every field here is a fixed-offset integer, so there is no equivalent of
/// cp-amm's `base_fee_kind` — the one property that can come back `None` from a
/// successful decode because its `BaseFeeMode` byte is an open enum. An
/// `LbPair` this function accepts yields all of its properties.
pub(in crate::application::decoder) fn decode_pool_account(
    data: &[u8],
) -> Result<DecodedPoolAccount, PoolAccountRejection> {
    const PROTOCOL: Protocol = Protocol::MeteoraDlmm;

    if data.len() < MIN_LEN {
        return Err(PoolAccountRejection::Truncated {
            protocol: PROTOCOL,
            len: data.len(),
            min: MIN_LEN,
        });
    }
    if data[..8] != LB_PAIR_DISCRIMINATOR {
        return Err(PoolAccountRejection::NotAPoolAccount { protocol: PROTOCOL });
    }

    // Every read below is in bounds: the length check above covers the whole
    // layout.
    let bin_step = u16_at(data, BIN_STEP_OFFSET);
    let base_factor = u16_at(data, BASE_FACTOR_OFFSET);
    let base_fee_power_factor = data[BASE_FEE_POWER_FACTOR_OFFSET];

    Ok(DecodedPoolAccount {
        registry: PoolRegistryProperties {
            token_a_mint: Pubkey::try_from(&data[TOKEN_X_MINT_OFFSET..TOKEN_X_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            token_b_mint: Pubkey::try_from(&data[TOKEN_Y_MINT_OFFSET..TOKEN_Y_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            // The base fee only — the floor a swapper pays, before the
            // volatility-driven part. The same notion cp-amm's cliff numerator
            // carries, which is what lets one `pools.fee_bps` mean one thing.
            fee_bps: base_fee_bps(base_factor, bin_step, base_fee_power_factor),
        },
        properties: PoolAccountProperties::MeteoraDlmm(MeteoraDlmmPoolAccountProperties {
            bin_step,
            base_factor,
            base_fee_power_factor,
            variable_fee_control: u32_at(data, VARIABLE_FEE_CONTROL_OFFSET),
            max_volatility_accumulator: u32_at(data, MAX_VOLATILITY_ACCUMULATOR_OFFSET),
            protocol_share: u16_at(data, PROTOCOL_SHARE_OFFSET),
        }),
    })
}
