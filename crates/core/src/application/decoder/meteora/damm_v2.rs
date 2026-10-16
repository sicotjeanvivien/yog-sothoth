//! Decoding of the cp-amm (Meteora DAMM v2) `Pool` account: an 8-byte Anchor
//! discriminator, then a zero-copy struct at fixed offsets (`state/pool.rs`).
//!
//! ⚠️ The genesis event's fee blob is borsh with `Option` tags: its offsets differ
//! (`base_fee_mode` at 26 there, 16 here). Never copy a constant across.

use solana_pubkey::Pubkey;

use crate::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams, fee_numerator_to_bps};
use crate::application::decoder::PoolAccountRejection;
use crate::domain::{
    DecodedPoolAccount, MeteoraDammV2PoolAccountProperties, PoolAccountProperties,
    PoolRegistryProperties, Protocol,
};

/// `sha256("account:Pool")[..8]`.
/// ⚠️ Not redundant with the owner dispatch: it refuses another account of the same program.
pub(in crate::application::decoder) const POOL_DISCRIMINATOR: [u8; 8] =
    [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];

/// `cliff_fee_numerator`: the leading `u64` of `pool_fees`.
pub(in crate::application::decoder) const CLIFF_FEE_NUMERATOR_OFFSET: usize = 8;
/// `BaseFeeMode` discriminant, at byte 8 of `BaseFeeInfo`: safe to read in every
/// variant, all three pod structs share their first 9 bytes.
pub(in crate::application::decoder) const BASE_FEE_MODE_OFFSET: usize = 16;

/// Scheduler period count (`u16` LE), at byte 14 of `BaseFeeInfo`.
/// ⚠️ Scheduler modes only: mode 2 puts `fee_increment_bps` here.
pub(in crate::application::decoder) const NUMBER_OF_PERIOD_OFFSET: usize = 22;

/// Fee-split percents (`u8` each).
/// ⚠️ Byte 49 between them is cp-amm's `padding_0`, not a partner fee.
pub(in crate::application::decoder) const PROTOCOL_FEE_PERCENT_OFFSET: usize = 48;
pub(in crate::application::decoder) const REFERRAL_FEE_PERCENT_OFFSET: usize = 50;

/// `DynamicFeeStruct::initialized`: a plain flag, **non-zero means enabled**.
pub(in crate::application::decoder) const DYNAMIC_FEE_INITIALIZED_OFFSET: usize = 56;
pub(in crate::application::decoder) const TOKEN_A_MINT_OFFSET: usize = 168;
pub(in crate::application::decoder) const TOKEN_B_MINT_OFFSET: usize = 200;

/// The other two members of `PodAlignedFeeTimeScheduler`.
/// ⚠️ Time schedulers (modes 0, 1) only: the other modes lay different fields over
/// these bytes. [`decode_fee_scheduler`] gates on the mode.
pub(in crate::application::decoder) const PERIOD_FREQUENCY_OFFSET: usize = 24;
pub(in crate::application::decoder) const REDUCTION_FACTOR_OFFSET: usize = 32;

/// `Pool::activation_point` (u64) and `Pool::activation_type` (u8,
/// **0 = slot, 1 = timestamp**, the unit of the point and of `period_frequency`).
pub(in crate::application::decoder) const ACTIVATION_POINT_OFFSET: usize = 472;
pub(in crate::application::decoder) const ACTIVATION_TYPE_OFFSET: usize = 480;

/// Minimum length for the **required** fields, the mints and the cliff fee.
/// ⚠️ Not the scheduler fields: refusing the account for an optional extra would
/// leave the pool unresolved, at the head of the queue forever.
const MIN_LEN: usize = TOKEN_B_MINT_OFFSET + 32;

/// Length needed for the fee-scheduler reads, checked separately from
/// [`MIN_LEN`] so a short account degrades instead of being refused.
const SCHEDULER_MIN_LEN: usize = ACTIVATION_TYPE_OFFSET + 1;

/// Decode a cp-amm `Pool` account, split by **who stores it**: the `pools`
/// registry (token pair, base fee) and this protocol's satellite.
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

    // ⚠️ An unknown mode is `None`, not a rejection: it must not fail the decode.
    let number_of_period = u16::from_le_bytes(
        data[NUMBER_OF_PERIOD_OFFSET..NUMBER_OF_PERIOD_OFFSET + 2]
            .try_into()
            .expect("2 bytes, length checked above"),
    );
    let base_fee_kind =
        crate::amm::damm_v2::base_fee_kind_from(data[BASE_FEE_MODE_OFFSET], number_of_period).ok();

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
/// ⚠️ The gate is the point: read under another mode, these bytes are other fields.
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
    if data.len() < SCHEDULER_MIN_LEN {
        return None;
    }

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
