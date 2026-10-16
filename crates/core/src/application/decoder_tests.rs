//! Pool-account decoding on synthetic buffers: fields come out of the right
//! offsets, and every non-pool input is rejected rather than mis-decoded.

use super::*;
use crate::amm::damm_v2::BaseFeeKind;
use crate::application::decoder::PoolAccountRejection;
use crate::application::decoder::meteora::damm_v2::{
    BASE_FEE_MODE_OFFSET, CLIFF_FEE_NUMERATOR_OFFSET, DYNAMIC_FEE_INITIALIZED_OFFSET,
    NUMBER_OF_PERIOD_OFFSET, POOL_DISCRIMINATOR, PROTOCOL_FEE_PERCENT_OFFSET,
    REFERRAL_FEE_PERCENT_OFFSET, TOKEN_A_MINT_OFFSET, TOKEN_B_MINT_OFFSET,
};
use crate::application::decoder::meteora::dlmm::{
    BASE_FACTOR_OFFSET, BASE_FEE_POWER_FACTOR_OFFSET, BIN_STEP_OFFSET, LB_PAIR_DISCRIMINATOR,
    MAX_VOLATILITY_ACCUMULATOR_OFFSET, PROTOCOL_SHARE_OFFSET, TOKEN_X_MINT_OFFSET,
    TOKEN_Y_MINT_OFFSET, VARIABLE_FEE_CONTROL_OFFSET,
};
use crate::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDlmmPoolAccountProperties, PoolAccountProperties,
};
use rust_decimal::Decimal;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

const CP_AMM_ACCOUNT_LEN: usize = 1112;

/// cp-amm's `padding_0`, named only so a test can poison it.
const PADDING_0_OFFSET: usize = 49;

/// A cp-amm `Pool` account with the discriminator, the cliff fee numerator, the
/// two fee-split percents and the two mints at their real offsets.
fn cp_amm_account(
    cliff_fee_numerator: u64,
    percents: (u8, u8),
    token_a: Pubkey,
    token_b: Pubkey,
) -> Vec<u8> {
    let mut bytes = vec![0u8; CP_AMM_ACCOUNT_LEN];
    bytes[..8].copy_from_slice(&POOL_DISCRIMINATOR);
    bytes[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
        .copy_from_slice(&cliff_fee_numerator.to_le_bytes());
    bytes[PROTOCOL_FEE_PERCENT_OFFSET] = percents.0;
    bytes[REFERRAL_FEE_PERCENT_OFFSET] = percents.1;
    bytes[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32].copy_from_slice(token_a.as_ref());
    bytes[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32].copy_from_slice(token_b.as_ref());
    bytes
}

/// Overwrite the fee-shape bytes of an account buffer: `BaseFeeMode`, the
/// scheduler period count, and the dynamic-fee flag.
fn with_fee_shape(mut bytes: Vec<u8>, mode: u8, number_of_period: u16, dynamic: bool) -> Vec<u8> {
    bytes[BASE_FEE_MODE_OFFSET] = mode;
    bytes[NUMBER_OF_PERIOD_OFFSET..NUMBER_OF_PERIOD_OFFSET + 2]
        .copy_from_slice(&number_of_period.to_le_bytes());
    bytes[DYNAMIC_FEE_INITIALIZED_OFFSET] = u8::from(dynamic);
    bytes
}

fn decode_shape(bytes: &[u8]) -> (Option<BaseFeeKind>, bool) {
    let props = damm_v2_properties(bytes);
    (props.base_fee_kind, props.has_dynamic_fee)
}

/// The cp-amm half of a successful decode; panics on another variant.
fn damm_v2_properties(bytes: &[u8]) -> MeteoraDammV2PoolAccountProperties {
    let properties = decode_pool_account(&cp_amm_owner(), bytes)
        .expect("should decode")
        .properties;

    match properties {
        PoolAccountProperties::MeteoraDammV2(props) => props,
        other => panic!("expected cp-amm properties, got {other:?}"),
    }
}

fn cp_amm_owner() -> Pubkey {
    Protocol::MeteoraDammV2.program_id()
}

// ── DLMM `LbPair` fixtures ──────────────────────────────────────────

const LB_PAIR_ACCOUNT_LEN: usize = 904;

/// A DLMM `LbPair` account with the discriminator, fee parameters and mints.
/// ⚠️ Mints distinct from the cp-amm fixtures', so a cross-decode cannot pass.
fn lb_pair_account(bin_step: u16, base_factor: u16, base_fee_power_factor: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; LB_PAIR_ACCOUNT_LEN];
    bytes[..8].copy_from_slice(&LB_PAIR_DISCRIMINATOR);
    bytes[BASE_FACTOR_OFFSET..BASE_FACTOR_OFFSET + 2].copy_from_slice(&base_factor.to_le_bytes());
    bytes[BASE_FEE_POWER_FACTOR_OFFSET] = base_fee_power_factor;
    bytes[BIN_STEP_OFFSET..BIN_STEP_OFFSET + 2].copy_from_slice(&bin_step.to_le_bytes());
    bytes[TOKEN_X_MINT_OFFSET..TOKEN_X_MINT_OFFSET + 32].copy_from_slice(pk(4).as_ref());
    bytes[TOKEN_Y_MINT_OFFSET..TOKEN_Y_MINT_OFFSET + 32].copy_from_slice(pk(5).as_ref());
    bytes
}

/// Overwrite the dynamic-fee parameters and Meteora's cut.
fn with_dlmm_fee_params(
    mut bytes: Vec<u8>,
    variable_fee_control: u32,
    max_volatility_accumulator: u32,
    protocol_share: u16,
) -> Vec<u8> {
    bytes[VARIABLE_FEE_CONTROL_OFFSET..VARIABLE_FEE_CONTROL_OFFSET + 4]
        .copy_from_slice(&variable_fee_control.to_le_bytes());
    bytes[MAX_VOLATILITY_ACCUMULATOR_OFFSET..MAX_VOLATILITY_ACCUMULATOR_OFFSET + 4]
        .copy_from_slice(&max_volatility_accumulator.to_le_bytes());
    bytes[PROTOCOL_SHARE_OFFSET..PROTOCOL_SHARE_OFFSET + 2]
        .copy_from_slice(&protocol_share.to_le_bytes());
    bytes
}

/// The DLMM half of a successful decode, unwrapped to its concrete type.
fn dlmm_properties(bytes: &[u8]) -> MeteoraDlmmPoolAccountProperties {
    let properties = decode_pool_account(&dlmm_owner(), bytes)
        .expect("should decode")
        .properties;

    match properties {
        PoolAccountProperties::MeteoraDlmm(props) => props,
        other => panic!("expected DLMM properties, got {other:?}"),
    }
}

fn dlmm_owner() -> Pubkey {
    Protocol::MeteoraDlmm.program_id()
}

// ── Happy path ──────────────────────────────────────────────────────

#[test]
fn decodes_cp_amm_fields_at_their_offsets() {
    // 2_500_000 / 1e9 = 0.25% = 25 bps; (protocol, referral) = (20, 20) —
    // a real mainnet constant-fee value and split.
    let data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    let decoded = decode_pool_account(&cp_amm_owner(), &data).expect("should decode");

    assert_eq!(decoded.protocol(), Protocol::MeteoraDammV2);
    // The neutral half — what the `pools` registry stores.
    assert_eq!(decoded.registry.token_a_mint, pk(2));
    assert_eq!(decoded.registry.token_b_mint, pk(3));
    assert_eq!(decoded.registry.fee_bps, Decimal::new(25, 0));
    // …and the cp-amm half, which goes to this protocol's satellite.
    let props = damm_v2_properties(&data);
    assert_eq!(props.protocol_fee_percent, 20);
    assert_eq!(props.referral_fee_percent, 20);
}

/// Mutation: reading byte 49 (`padding_0`) into any field; poisoned, it must change nothing.
#[test]
fn padding_between_the_percents_is_not_decoded() {
    let mut data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));
    data[PADDING_0_OFFSET] = 0xFF;

    let props = damm_v2_properties(&data);
    assert_eq!(props.protocol_fee_percent, 20, "byte 48 must be untouched");
    assert_eq!(props.referral_fee_percent, 20, "byte 50 must be untouched");
}

// ── Fee shape ───────────────────────────────────────────────────────

/// Every `BaseFeeMode`, each scheduler both ways.
/// Mutation: deciding on the mode byte alone, ignoring the period count.
#[test]
fn every_base_fee_mode_maps_to_its_kind() {
    let base = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    for (mode, periods, expected) in [
        (0u8, 0u16, BaseFeeKind::Constant),
        (0, 12, BaseFeeKind::SchedulerLinear),
        (1, 0, BaseFeeKind::Constant),
        (1, 12, BaseFeeKind::SchedulerExponential),
        (3, 0, BaseFeeKind::Constant),
        (3, 12, BaseFeeKind::MarketCapSchedulerLinear),
        (4, 0, BaseFeeKind::Constant),
        (4, 12, BaseFeeKind::MarketCapSchedulerExponential),
    ] {
        let data = with_fee_shape(base.clone(), mode, periods, false);
        assert_eq!(
            decode_shape(&data).0,
            Some(expected),
            "mode {mode} with {periods} periods"
        );
    }
}

/// Mutation: the rate limiter consulting the period count, its `fee_increment_bps`.
#[test]
fn the_rate_limiter_ignores_the_period_count() {
    let base = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    for bytes_at_period_offset in [0u16, 250] {
        let data = with_fee_shape(base.clone(), 2, bytes_at_period_offset, false);
        assert_eq!(decode_shape(&data).0, Some(BaseFeeKind::RateLimiter));
    }
}

/// Mutation: rejecting the account on an unknown `BaseFeeMode`, which strands
/// the pool at the head of the unresolved queue; only the fee shape may be lost.
#[test]
fn an_unknown_base_fee_mode_costs_only_the_fee_shape() {
    let data = with_fee_shape(
        cp_amm_account(2_500_000, (20, 20), pk(2), pk(3)),
        99,
        0,
        true,
    );

    let decoded = decode_pool_account(&cp_amm_owner(), &data).expect("must still decode");

    assert_eq!(
        decoded.registry.token_a_mint,
        pk(2),
        "the mints must survive"
    );
    assert_eq!(
        decoded.registry.fee_bps,
        Decimal::new(25, 0),
        "the tier must survive"
    );
    let props = damm_v2_properties(&data);
    assert_eq!(props.base_fee_kind, None, "the unmappable mode yields None");
    assert!(props.has_dynamic_fee, "the flag is independent of the mode");
}

/// Mutation: `!= 0` → `== 1`; any non-zero `dynamic_fee.initialized` means enabled.
#[test]
fn the_dynamic_fee_flag_is_any_non_zero_byte() {
    let base = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    assert!(!decode_shape(&with_fee_shape(base.clone(), 0, 0, false)).1);
    assert!(decode_shape(&with_fee_shape(base.clone(), 0, 0, true)).1);

    let mut odd = with_fee_shape(base, 0, 0, false);
    odd[DYNAMIC_FEE_INITIALIZED_OFFSET] = 7;
    assert!(decode_shape(&odd).1, "non-zero is enabled, not just 1");
}

// ── The two guards ──────────────────────────────────────────────────

// Each rejection is asserted by *variant*, not merely as "not decoded".

#[test]
fn an_unindexed_program_is_rejected_as_such() {
    let data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    assert_eq!(
        decode_pool_account(&pk(99), &data),
        Err(PoolAccountRejection::UnknownProgram { program_id: pk(99) })
    );
}

#[test]
fn wrong_discriminator_is_rejected_as_not_a_pool_account() {
    let mut data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));
    data[..8].fill(0);

    assert_eq!(
        decode_pool_account(&cp_amm_owner(), &data),
        Err(PoolAccountRejection::NotAPoolAccount {
            protocol: Protocol::MeteoraDammV2
        })
    );
}

/// Mutation: reporting a short account as the wrong discriminator.
#[test]
fn a_short_account_is_rejected_as_truncated_with_its_sizes() {
    let err = decode_pool_account(&cp_amm_owner(), &[0u8; 16]).expect_err("should reject");

    match err {
        PoolAccountRejection::Truncated { protocol, len, min } => {
            assert_eq!(protocol, Protocol::MeteoraDammV2);
            assert_eq!(len, 16);
            assert!(min > len, "the layout must need more than we were given");
        }
        other => panic!("expected Truncated, got {other:?}"),
    }
}

/// A DLMM `LbPair` holds valid `Pubkey`s at cp-amm's mint offsets.
/// Mutation: dropping either guard, the owner dispatch or the discriminator.
#[test]
fn foreign_account_is_not_decoded_at_our_offsets() {
    let mut lb_pair = lb_pair_account(1, 10_000, 0);
    // The two reserve accounts, where cp-amm would look for mints.
    lb_pair[152..184].copy_from_slice(pk(77).as_ref()); // reserve_x
    lb_pair[184..216].copy_from_slice(pk(88).as_ref()); // reserve_y

    // Routed to its own decoder, the account yields DLMM properties…
    assert_eq!(
        decode_pool_account(&dlmm_owner(), &lb_pair)
            .expect("should decode")
            .protocol(),
        Protocol::MeteoraDlmm
    );

    // …and the cp-amm decoder still rejects it on the discriminator.
    assert_eq!(
        super::meteora::damm_v2::decode_pool_account(&lb_pair),
        Err(PoolAccountRejection::NotAPoolAccount {
            protocol: Protocol::MeteoraDammV2
        }),
        "cp-amm decoder must reject an LbPair even if handed one directly"
    );
}

// ── DLMM `LbPair` ───────────────────────────────────────────────────

/// The parameters of pool `HTvjzs…` (SOL/USDC), which Meteora shows at 0.01 %:
/// observed values, not chosen ones.
#[test]
fn decodes_lb_pair_fields_at_their_offsets() {
    let data = with_dlmm_fee_params(lb_pair_account(1, 10_000, 0), 2_000_000, 100_000, 1_000);

    let decoded = decode_pool_account(&dlmm_owner(), &data).expect("should decode");

    assert_eq!(decoded.protocol(), Protocol::MeteoraDlmm);
    // The neutral half — what the `pools` registry stores.
    assert_eq!(decoded.registry.token_a_mint, pk(4));
    assert_eq!(decoded.registry.token_b_mint, pk(5));
    assert_eq!(
        decoded.registry.fee_bps,
        Decimal::ONE,
        "10000 x 1 x 10^0 / 10000 = 1 bps, what Meteora displays for this pool"
    );
    // …and the DLMM half, which goes to this protocol's satellite.
    let props = dlmm_properties(&data);
    assert_eq!(props.bin_step, 1);
    assert_eq!(props.base_factor, 10_000);
    assert_eq!(props.base_fee_power_factor, 0);
    assert_eq!(props.variable_fee_control, 2_000_000);
    assert_eq!(props.max_volatility_accumulator, 100_000);
    assert_eq!(props.protocol_share, 1_000);
}

/// Mutation: an off-by-one onto the adjacent `active_id` or `filter_period`.
#[test]
fn the_fee_inputs_do_not_bleed_into_each_other() {
    let props = dlmm_properties(&lb_pair_account(25, 5_000, 2));

    assert_eq!(props.bin_step, 25);
    assert_eq!(props.base_factor, 5_000);
    assert_eq!(props.base_fee_power_factor, 2);
}

/// Mutation: reading `active_id`, per-swap state; poisoned, it must change nothing.
#[test]
fn state_bytes_are_not_decoded_into_the_satellite() {
    let baseline = dlmm_properties(&lb_pair_account(1, 10_000, 0));

    let mut poisoned = lb_pair_account(1, 10_000, 0);
    poisoned[76..80].copy_from_slice(&(-26_085i32).to_le_bytes()); // active_id
    poisoned[40..72].fill(0xFF); // the whole VariableParameters block

    assert_eq!(
        dlmm_properties(&poisoned),
        baseline,
        "only StaticParameters and bin_step may reach the satellite"
    );
}

/// No variable fee is a zero magnitude: this layout has no flag for it.
#[test]
fn a_zero_variable_fee_control_means_no_dynamic_fee() {
    let props = dlmm_properties(&with_dlmm_fee_params(
        lb_pair_account(1, 10_000, 0),
        0,
        100_000,
        1_000,
    ));

    assert_eq!(props.variable_fee_control, 0);
}

/// Mutation: a narrowing read of any field; the full `u32` range must survive.
#[test]
fn the_dynamic_fee_parameters_keep_their_full_range() {
    let props = dlmm_properties(&with_dlmm_fee_params(
        lb_pair_account(u16::MAX, u16::MAX, 0),
        u32::MAX,
        u32::MAX,
        u16::MAX,
    ));

    assert_eq!(props.bin_step, u16::MAX);
    assert_eq!(props.base_factor, u16::MAX);
    assert_eq!(props.base_fee_power_factor, 0, "not set by this fixture");
    assert_eq!(props.variable_fee_control, u32::MAX);
    assert_eq!(props.max_volatility_accumulator, u32::MAX);
    assert_eq!(props.protocol_share, u16::MAX);
}

/// Mutation: dropping the DLMM discriminator check; a cp-amm `Pool` yields valid
/// `Pubkey`s at DLMM's mint offsets.
#[test]
fn cp_amm_account_is_rejected_by_the_dlmm_decoder() {
    let cp_amm = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    assert_eq!(
        decode_pool_account(&dlmm_owner(), &cp_amm),
        Err(PoolAccountRejection::NotAPoolAccount {
            protocol: Protocol::MeteoraDlmm
        })
    );
}

#[test]
fn a_short_lb_pair_is_rejected_as_truncated_with_its_sizes() {
    let short = lb_pair_account(1, 10_000, 0)[..151].to_vec();

    assert_eq!(
        decode_pool_account(&dlmm_owner(), &short),
        Err(PoolAccountRejection::Truncated {
            protocol: Protocol::MeteoraDlmm,
            len: 151,
            min: 152,
        })
    );
}

// ── Protocol::from_program_id ───────────────────────────────────────

#[test]
fn every_protocol_round_trips_through_its_program_id() {
    for protocol in Protocol::all() {
        assert_eq!(
            Protocol::from_program_id(&protocol.program_id()),
            Some(*protocol)
        );
    }
}

#[test]
fn an_unindexed_program_id_maps_to_no_protocol() {
    assert_eq!(Protocol::from_program_id(&pk(99)), None);
}
