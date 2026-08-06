//! Decoding of the cp-amm (Meteora DAMM v2) `Pool` account.
//!
//! # Layout
//!
//! An 8-byte Anchor discriminator, then a zero-copy struct at fixed offsets.
//! Derived from cp-amm's own `state/fee.rs` and `state/pool.rs`, **not** guessed
//! from samples:
//!
//! ```text
//! 8    pool_fees : PoolFeesStruct (160 bytes, 8..168)
//!      8    base_fee : BaseFeeStruct (40 bytes)
//!           8    cliff_fee_numerator   u64   ← the base fee numerator
//!           16   base_fee_mode         u8
//!           22   number_of_period      u16
//!           24   period_frequency      u64   ← time schedulers only
//!           32   reduction_factor      u64   ← time schedulers only
//!      48   protocol_fee_percent       u8
//!      49   padding_0                  u8    ← NOT a partner fee, see below
//!      50   referral_fee_percent       u8
//!      56   dynamic_fee.initialized    u8
//! 168  token_a_mint     Pubkey
//! 200  token_b_mint     Pubkey
//! 472  activation_point u64             ← slot or unix ts, see activation_type
//! 480  activation_type  u8              ← 0 = slot, 1 = timestamp
//! ```
//!
//! The offsets past the mints follow from `Pool`'s declaration order: three more
//! `Pubkey`s (vaults, whitelisted vault), a `[u8; 32]` padding, then `liquidity`,
//! a padding, two `u64` protocol fees, a padding and the three `sqrt_*` prices —
//! all `u128` — which lands `activation_point` at 472. Confirmed on the eleven
//! captured accounts, where it reads as a mid-2026 Unix timestamp every time.
//!
//! The account is 1112 bytes long.
//!
//! # Do not transpose the event-blob offsets
//!
//! `amm::damm_v2` decodes the same *concepts* from `pool_fees_raw`, at different
//! offsets: that blob is a borsh `PoolFeeParameters` whose `Option` fields are
//! variable-length (its dynamic-fee tag moves between byte 1 and byte 9). This
//! account is a zero-copy struct with no `Option` tags. `base_fee_mode` is at 26
//! in the blob and 16 here. Copying a constant across would decode silently
//! wrong.
//!
//! # There is no partner fee
//!
//! Byte 49 was read as `partner_fee_percent` until migration 037. cp-amm
//! declares it `padding_0`, and the word "partner" appears nowhere in its
//! `state/fee.rs`. The neighbouring offsets (48, 50) are correct, which is what
//! hid it: two of three percents decoded fine, and the third was always 0 —
//! plausible for a partner cut, inevitable for padding.
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

use crate::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams, fee_numerator_to_bps};
use crate::application::decoder::PoolAccountRejection;
use crate::domain::{
    DecodedPoolAccount, MeteoraDammV2PoolAccountProperties, PoolAccountProperties,
    PoolRegistryProperties, Protocol,
};

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
/// `BaseFeeMode` discriminant, at byte 8 of `BaseFeeInfo`.
///
/// `BaseFeeInfo` is a `[u8; 32]` reinterpreted as one of three pod structs
/// (time scheduler, rate limiter, market-cap scheduler). All three share the
/// same first 9 bytes — `cliff_fee_numerator` then `base_fee_mode` — which is
/// what makes reading the mode safe without knowing which variant it is.
pub(in crate::application::decoder) const BASE_FEE_MODE_OFFSET: usize = 16;

/// Scheduler period count (`u16`, little-endian), at byte 14 of `BaseFeeInfo`.
///
/// **Only meaningful for the scheduler modes (0, 1, 3, 4)**, whose three pod
/// variants all place `number_of_period` here. Mode 2 (rate limiter) puts
/// `fee_increment_bps` at the same spot — [`yog_core`'s mapping] never consults
/// this value for that mode.
///
/// [`yog_core`'s mapping]: crate::amm::damm_v2::base_fee_kind_from
pub(in crate::application::decoder) const NUMBER_OF_PERIOD_OFFSET: usize = 22;

/// Fee-split percents (`u8` each), after the 40-byte `BaseFeeStruct` inside
/// `PoolFeesStruct`. **Not adjacent**: `padding_0` sits at 49 between them.
pub(in crate::application::decoder) const PROTOCOL_FEE_PERCENT_OFFSET: usize = 48;
pub(in crate::application::decoder) const REFERRAL_FEE_PERCENT_OFFSET: usize = 50;

/// `DynamicFeeStruct::initialized`, the first byte of the dynamic-fee block.
///
/// A plain flag, not a borsh `Option` tag: **non-zero means enabled**. Unlike
/// the genesis event's blob — where the same fact is an `Option` tag whose
/// position shifts with what precedes it — this byte is at a fixed offset and
/// carries no tri-state.
pub(in crate::application::decoder) const DYNAMIC_FEE_INITIALIZED_OFFSET: usize = 56;
pub(in crate::application::decoder) const TOKEN_A_MINT_OFFSET: usize = 168;
pub(in crate::application::decoder) const TOKEN_B_MINT_OFFSET: usize = 200;

/// The remaining two members of `PodAlignedFeeTimeScheduler`, needed to place a
/// pool's base fee on its decay curve rather than only at its start.
///
/// ⚠️ **Valid for the time-scheduler modes only (0 and 1).** `BaseFeeInfo` is a
/// 32-byte region the modes reinterpret: mode 2 (`rate_limiter`) and modes 3/4
/// (market-cap schedulers) lay different fields over these very bytes. Read
/// blindly they return nonsense, and this is measured rather than feared — on
/// the captured fixtures, mode 4 yields a `period_frequency` of
/// 13 722 280 043 814 587 382 and mode 2 one of 42 520 176 273 600.
/// [`decode_fee_scheduler`] is what gates on the mode.
pub(in crate::application::decoder) const PERIOD_FREQUENCY_OFFSET: usize = 24;
pub(in crate::application::decoder) const REDUCTION_FACTOR_OFFSET: usize = 32;

/// `Pool::activation_point` (u64) and `Pool::activation_type` (u8), which sit
/// well past the mints — after `sqrt_price`, itself after five `Pubkey`s, a
/// 32-byte padding and four `u128`s.
///
/// `activation_type` names the unit of both `activation_point` and
/// `period_frequency`: **0 = slot, 1 = timestamp**. All eleven captured mainnet
/// accounts use 1, so the slot branch is real but unexercised by the fixtures.
pub(in crate::application::decoder) const ACTIVATION_POINT_OFFSET: usize = 472;
pub(in crate::application::decoder) const ACTIVATION_TYPE_OFFSET: usize = 480;

/// Minimum length for every field above to be in bounds.
const MIN_LEN: usize = ACTIVATION_TYPE_OFFSET + 1;

/// Decode a cp-amm `Pool` account.
///
/// The caller has already routed on the program id, so this carries the second
/// of the two guards described in [`super::super`] — the discriminator — and
/// distinguishes it from a truncated account, because the two mean very
/// different things: a wrong discriminator is the wrong account, a short one is
/// most likely an ABI change.
/// Returns the read split by **who stores it**: the neutral token pair and base
/// fee, which the `pools` registry owns, and the cp-amm-only properties, which
/// this protocol's satellite owns. One read of one buffer, two writers.
pub(in crate::application::decoder) fn decode_pool_account(
    data: &[u8],
) -> Result<DecodedPoolAccount, PoolAccountRejection> {
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

    // The fee *shape*. Unknown mode → `None` rather than a rejection: see the
    // note on `base_fee_kind` below for why this one property must not be able
    // to fail the whole decode.
    let number_of_period = u16::from_le_bytes(
        data[NUMBER_OF_PERIOD_OFFSET..NUMBER_OF_PERIOD_OFFSET + 2]
            .try_into()
            .expect("2 bytes, length checked above"),
    );
    let base_fee_kind =
        crate::amm::damm_v2::base_fee_kind_from(data[BASE_FEE_MODE_OFFSET], number_of_period).ok();

    // Every slice below is in bounds: the length check above covers the whole
    // layout, so these conversions cannot fail.
    let cliff_fee_numerator = u64::from_le_bytes(
        data[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
            .try_into()
            .expect("8 bytes, length checked above"),
    );

    Ok(DecodedPoolAccount {
        registry: PoolRegistryProperties {
            token_a_mint: Pubkey::try_from(&data[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            token_b_mint: Pubkey::try_from(&data[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32])
                .expect("32 bytes, length checked above"),
            fee_bps: fee_numerator_to_bps(cliff_fee_numerator),
        },
        properties: PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
            protocol_fee_percent: data[PROTOCOL_FEE_PERCENT_OFFSET],
            referral_fee_percent: data[REFERRAL_FEE_PERCENT_OFFSET],
            base_fee_kind,
            has_dynamic_fee: data[DYNAMIC_FEE_INITIALIZED_OFFSET] != 0,
            fee_scheduler: decode_fee_scheduler(data, base_fee_kind, number_of_period),
        }),
    })
}

/// The time-scheduler parameters, **or `None` for every other fee shape**.
///
/// The gate is the whole point. `BaseFeeInfo` is 32 bytes the modes reinterpret,
/// so `period_frequency` and `reduction_factor` only mean what they say for
/// modes 0 and 1; for the others their bytes belong to different fields. A
/// decoder that read them unconditionally would publish a confident, absurd
/// fee curve for the 8 market-cap and rate-limiter pools rather than admitting
/// it does not model them.
///
/// `Constant` is excluded too, for a simpler reason: it does not decay, so
/// `fee_bps` already tells the whole truth about it.
fn decode_fee_scheduler(
    data: &[u8],
    base_fee_kind: Option<BaseFeeKind>,
    number_of_period: u16,
) -> Option<FeeSchedulerParams> {
    let kind = base_fee_kind?;
    if !matches!(
        kind,
        BaseFeeKind::SchedulerLinear | BaseFeeKind::SchedulerExponential
    ) {
        return None;
    }

    // In bounds: `MIN_LEN` covers through `ACTIVATION_TYPE_OFFSET`.
    let read_u64 = |at: usize| {
        u64::from_le_bytes(
            data[at..at + 8]
                .try_into()
                .expect("8 bytes, length checked by the caller"),
        )
    };

    Some(FeeSchedulerParams {
        cliff_fee_numerator: read_u64(CLIFF_FEE_NUMERATOR_OFFSET),
        number_of_period,
        period_frequency: read_u64(PERIOD_FREQUENCY_OFFSET),
        reduction_factor: read_u64(REDUCTION_FACTOR_OFFSET),
        activation_point: read_u64(ACTIVATION_POINT_OFFSET),
        activation_type: data[ACTIVATION_TYPE_OFFSET],
        kind,
    })
}
