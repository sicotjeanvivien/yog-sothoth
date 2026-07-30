//! Unit tests for pool-account decoding.
//!
//! No network and no base64: build a synthetic account buffer matching the
//! cp-amm `Pool` layout and assert the fields come out of the right offsets,
//! and that every non-pool input is rejected rather than mis-decoded.
//!
//! Ported from `context/src/providers/cpamm_pool_tests.rs` when the decoder
//! moved into `core`, plus the case that motivates the whole guard discipline
//! (`foreign_account_is_not_decoded_at_our_offsets`).

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

/// Byte 49 of the account: cp-amm's `padding_0`, between `protocol_fee_percent`
/// and `referral_fee_percent`. Named here only so the regression test below can
/// poison it — the decoder must have no constant for it.
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

/// The cp-amm half of a successful decode, unwrapped to its concrete type.
///
/// Unwrapping through a `match` rather than an irrefutable `let`: the enum has
/// more than one variant now, and a decode landing on another one is a routing
/// bug the assertions below would otherwise report as a confusing field
/// mismatch.
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

/// A DLMM `LbPair` account with the discriminator, the fee parameters and the
/// two mints at their real offsets. Mints are `pk(4)` / `pk(5)`, chosen distinct
/// from the cp-amm fixtures' so a cross-decode cannot pass by coincidence.
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

/// Byte 49 is cp-amm's `padding_0`, not a partner fee (migration 037). Poison it
/// and assert **nothing** changes: the two real percents keep their values and
/// the padding leaks into no field.
///
/// The regression this pins is not "wrong value" but "field with no referent".
/// It went unnoticed for a month because the decoded value was always 0, which
/// is exactly what a plausible partner cut looks like — so the guard has to be
/// on the decoder reading the byte at all, not on what it produced.
#[test]
fn padding_between_the_percents_is_not_decoded() {
    let mut data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));
    data[PADDING_0_OFFSET] = 0xFF;

    let props = damm_v2_properties(&data);
    assert_eq!(props.protocol_fee_percent, 20, "byte 48 must be untouched");
    assert_eq!(props.referral_fee_percent, 20, "byte 50 must be untouched");
}

// ── Fee shape ───────────────────────────────────────────────────────

/// Every `BaseFeeMode` cp-amm defines, including the two market-cap schedulers.
///
/// The period count is what separates a constant fee from a decaying one, so
/// each scheduler mode is asserted **both ways** — the mode byte alone is not
/// the answer.
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

/// The rate limiter is the one mode that must **not** consult the period count:
/// its layout puts `fee_increment_bps` at those bytes. A non-zero value there is
/// therefore meaningless, and must not turn it into a "constant" fee.
#[test]
fn the_rate_limiter_ignores_the_period_count() {
    let base = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    for bytes_at_period_offset in [0u16, 250] {
        let data = with_fee_shape(base.clone(), 2, bytes_at_period_offset, false);
        assert_eq!(decode_shape(&data).0, Some(BaseFeeKind::RateLimiter));
    }
}

/// **The starvation guard.** An unknown `BaseFeeMode` — a mode cp-amm adds after
/// this build — must cost the fee shape and *nothing else*: the account still
/// decodes, so the mints and the fee tier still land.
///
/// Rejecting the account instead would strand the pool in
/// `list_unresolved` forever, and since that queue is ordered by `first_seen_at`
/// and capped, such pools would accumulate at its head and starve every pool
/// behind them.
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

/// `dynamic_fee.initialized` is a flag, not a borsh `Option` tag: any non-zero
/// value means enabled. It sits at a fixed offset, so unlike the genesis blob it
/// cannot move with what precedes it.
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

// Each rejection is asserted by *variant*, not merely as "not decoded": the
// whole point of the typed rejection is that these four situations mean
// different things and must stay distinguishable to whoever logs them.

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

/// Distinct from the discriminator case on purpose: a short account is the
/// signature of an ABI change, and that must not read the same as "wrong
/// account".
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

/// **The case the guards exist for.** A DLMM `LbPair` account holds valid,
/// aligned `Pubkey`s exactly where cp-amm keeps its mints — `reserve_x` at 168
/// and `reserve_y` at 200. Decoding it at cp-amm's offsets would therefore
/// *succeed* and write the pool's vault addresses into its mint columns:
/// plausible, silently wrong data rather than an error.
///
/// Here the owner dispatch rejects it first; the discriminator check is the
/// second line. Both must stay.
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

    // …and the cp-amm decoder still rejects it on the discriminator, which is
    // the guard that matters: the reserves above sit exactly where cp-amm reads
    // its mints, so without it they would be written as the token pair.
    assert_eq!(
        super::meteora::damm_v2::decode_pool_account(&lb_pair),
        Err(PoolAccountRejection::NotAPoolAccount {
            protocol: Protocol::MeteoraDammV2
        }),
        "cp-amm decoder must reject an LbPair even if handed one directly"
    );
}

// ── DLMM `LbPair` ───────────────────────────────────────────────────

/// The offsets, against the values read from a live account before the decoder
/// existed: pool `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` (SOL/USDC) has
/// `base_factor = 10000`, `bin_step = 1`, `base_fee_power_factor = 0`,
/// `variable_fee_control = 2000000`, `max_volatility_accumulator = 100000`,
/// `protocol_share = 1000` — and Meteora shows 0.01 % for it, which is the
/// 1 bps asserted below.
///
/// This is what ties the layout to the chain rather than to a self-consistent
/// fixture: every constant here was observed, not chosen.
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

/// `bin_step` and `base_factor` are adjacent to fields this decoder ignores
/// (`active_id` at 76, `filter_period` at 10). Distinct non-zero values in each
/// catch an off-by-one that a fixture of equal values would hide.
#[test]
fn the_fee_inputs_do_not_bleed_into_each_other() {
    let props = dlmm_properties(&lb_pair_account(25, 5_000, 2));

    assert_eq!(props.bin_step, 25);
    assert_eq!(props.base_factor, 5_000);
    assert_eq!(props.base_fee_power_factor, 2);
}

/// `active_id` is state, not configuration — it moves on every swap. Poison its
/// bytes and assert nothing this decoder produces changes: it belongs to
/// `pool_current_state`, and reading it here would make the satellite churn.
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

/// A DLMM pool with no variable fee is expressed by a zero magnitude, not by a
/// flag — there is no `has_dynamic_fee` byte in this layout.
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

/// The full `u32` range must survive — `variable_fee_control` and
/// `max_volatility_accumulator` are stored in signed SQL columns, so a lossy
/// conversion would surface here first.
#[test]
fn the_dynamic_fee_parameters_keep_their_full_range() {
    let props = dlmm_properties(&with_dlmm_fee_params(
        lb_pair_account(u16::MAX, u16::MAX, 0),
        u32::MAX,
        u32::MAX,
        u16::MAX,
    ));

    assert_eq!(props.variable_fee_control, u32::MAX);
    assert_eq!(props.max_volatility_accumulator, u32::MAX);
    assert_eq!(props.protocol_share, u16::MAX);
    assert_eq!(props.bin_step, u16::MAX);
}

/// The mirror of the cp-amm guard: a cp-amm `Pool` handed to the DLMM decoder
/// must be rejected on the discriminator. At DLMM's mint offsets (88, 120) a
/// cp-amm account holds parts of its `pool_fees` block — bytes that would decode
/// as perfectly valid `Pubkey`s.
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

// ── Protocols recognized but not decoded yet ────────────────────────

/// A known protocol without a decoder is a **coverage gap**, and must not be
/// reported as an unindexed program — the two call for different reactions.
///
/// DAMM v1 is the only one left: DLMM gained its decoder with migration 039.
#[test]
fn known_protocol_without_a_decoder_is_rejected_as_a_coverage_gap() {
    let protocol = Protocol::MeteoraDammV1;
    assert_eq!(
        decode_pool_account(&protocol.program_id(), &[0u8; 1112]),
        Err(PoolAccountRejection::NoDecoder { protocol }),
        "{protocol} should be a coverage gap, not an unknown program"
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
