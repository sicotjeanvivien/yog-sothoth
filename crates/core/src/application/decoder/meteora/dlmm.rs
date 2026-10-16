//! Decoding of the Meteora DLMM (Liquidity Book) `LbPair` account: an 8-byte
//! Anchor discriminator, then a zero-copy struct at fixed offsets.
//! `active_id` and `VariableParameters` are per-swap state, not decoded here.

use solana_pubkey::Pubkey;

use crate::amm::dlmm::base_fee_bps;
use crate::application::decoder::PoolAccountRejection;
use crate::domain::{
    DecodedPoolAccount, MeteoraDlmmPoolAccountProperties, PoolAccountProperties,
    PoolRegistryProperties, Protocol,
};

/// `sha256("account:LbPair")[..8]`.
/// ⚠️ Not redundant with the owner dispatch: lb_clmm owns several account kinds.
pub(in crate::application::decoder) const LB_PAIR_DISCRIMINATOR: [u8; 8] =
    [0x21, 0x0b, 0x31, 0x62, 0xb5, 0x65, 0xb1, 0x0d];

/// `base_factor`, the leading `u16` of `StaticParameters`.
pub(in crate::application::decoder) const BASE_FACTOR_OFFSET: usize = 8;
/// Dynamic-fee magnitude and its per-pool ceiling; `variable_fee_control == 0`
/// means no variable fee (DLMM has no flag for it).
pub(in crate::application::decoder) const VARIABLE_FEE_CONTROL_OFFSET: usize = 16;
pub(in crate::application::decoder) const MAX_VOLATILITY_ACCUMULATOR_OFFSET: usize = 20;
/// Meteora's cut, in **basis points** — not the whole percent cp-amm uses.
pub(in crate::application::decoder) const PROTOCOL_SHARE_OFFSET: usize = 32;
/// Power-of-ten scaling on the base fee.
pub(in crate::application::decoder) const BASE_FEE_POWER_FACTOR_OFFSET: usize = 34;
pub(in crate::application::decoder) const BIN_STEP_OFFSET: usize = 80;
pub(in crate::application::decoder) const TOKEN_X_MINT_OFFSET: usize = 88;
pub(in crate::application::decoder) const TOKEN_Y_MINT_OFFSET: usize = 120;

/// Minimum length for every field above to be in bounds.
const MIN_LEN: usize = TOKEN_Y_MINT_OFFSET + 32;

/// Little-endian `u16` at `offset`; the caller has length-checked the layout.
fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        data[offset..offset + 2]
            .try_into()
            .expect("2 bytes, length checked by the caller"),
    )
}

/// Little-endian `u32` at `offset`, same contract as [`u16_at`].
fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("4 bytes, length checked by the caller"),
    )
}

/// Decode a DLMM `LbPair` account, split by **who stores it**: the `pools`
/// registry (token pair, base fee) and this protocol's satellite.
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

    let bin_step = u16_at(data, BIN_STEP_OFFSET);
    let base_factor = u16_at(data, BASE_FACTOR_OFFSET);
    let base_fee_power_factor = data[BASE_FEE_POWER_FACTOR_OFFSET];

    Ok(DecodedPoolAccount {
        registry: PoolRegistryProperties {
            token_a_mint: Pubkey::try_from(&data[TOKEN_X_MINT_OFFSET..TOKEN_X_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            token_b_mint: Pubkey::try_from(&data[TOKEN_Y_MINT_OFFSET..TOKEN_Y_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            // The floor, before the variable part: what cp-amm's `fee_bps` means too.
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
