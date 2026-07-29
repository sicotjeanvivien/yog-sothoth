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
use crate::application::decoder::PoolAccountRejection;
use crate::application::decoder::meteora::damm_v2::{
    CLIFF_FEE_NUMERATOR_OFFSET, POOL_DISCRIMINATOR, PROTOCOL_FEE_PERCENT_OFFSET,
    REFERRAL_FEE_PERCENT_OFFSET, TOKEN_A_MINT_OFFSET, TOKEN_B_MINT_OFFSET,
};
use crate::domain::PoolAccountProperties;
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

fn cp_amm_owner() -> Pubkey {
    Protocol::MeteoraDammV2.program_id()
}

// ── Happy path ──────────────────────────────────────────────────────

#[test]
fn decodes_cp_amm_fields_at_their_offsets() {
    // 2_500_000 / 1e9 = 0.25% = 25 bps; (protocol, referral) = (20, 20) —
    // a real mainnet constant-fee value and split.
    let data = cp_amm_account(2_500_000, (20, 20), pk(2), pk(3));

    let decoded = decode_pool_account(&cp_amm_owner(), &data).expect("should decode");

    assert_eq!(decoded.protocol(), Protocol::MeteoraDammV2);
    let PoolAccountProperties::MeteoraDammV2(props) = decoded;
    assert_eq!(props.token_a_mint, pk(2));
    assert_eq!(props.token_b_mint, pk(3));
    assert_eq!(props.fee_bps, Decimal::new(25, 0));
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

    let decoded = decode_pool_account(&cp_amm_owner(), &data).expect("should decode");

    let PoolAccountProperties::MeteoraDammV2(props) = decoded;
    assert_eq!(props.protocol_fee_percent, 20, "byte 48 must be untouched");
    assert_eq!(props.referral_fee_percent, 20, "byte 50 must be untouched");
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
    let mut lb_pair = vec![0u8; 904];
    // LbPair's own discriminator, and the two reserve accounts where cp-amm
    // would look for mints.
    lb_pair[..8].copy_from_slice(&[33, 11, 49, 98, 181, 101, 177, 13]);
    lb_pair[152..184].copy_from_slice(pk(77).as_ref()); // reserve_x
    lb_pair[184..216].copy_from_slice(pk(88).as_ref()); // reserve_y

    // Rejected on the program id…
    assert_eq!(
        decode_pool_account(&Protocol::MeteoraDlmm.program_id(), &lb_pair),
        Err(PoolAccountRejection::NoDecoder {
            protocol: Protocol::MeteoraDlmm
        })
    );

    // …and, were it ever routed here, on the discriminator too.
    assert_eq!(
        super::meteora::damm_v2::decode_pool_account(&lb_pair),
        Err(PoolAccountRejection::NotAPoolAccount {
            protocol: Protocol::MeteoraDammV2
        }),
        "cp-amm decoder must reject an LbPair even if handed one directly"
    );
}

// ── Protocols recognized but not decoded yet ────────────────────────

/// A known protocol without a decoder is a **coverage gap**, and must not be
/// reported as an unindexed program — the two call for different reactions.
#[test]
fn known_protocol_without_a_decoder_is_rejected_as_a_coverage_gap() {
    for protocol in [Protocol::MeteoraDammV1, Protocol::MeteoraDlmm] {
        assert_eq!(
            decode_pool_account(&protocol.program_id(), &[0u8; 1112]),
            Err(PoolAccountRejection::NoDecoder { protocol }),
            "{protocol} should be a coverage gap, not an unknown program"
        );
    }
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
