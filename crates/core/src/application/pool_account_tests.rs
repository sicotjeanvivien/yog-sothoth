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
use crate::application::pool_account::meteora::damm_v2::{
    CLIFF_FEE_NUMERATOR_OFFSET, PARTNER_FEE_PERCENT_OFFSET, POOL_DISCRIMINATOR,
    PROTOCOL_FEE_PERCENT_OFFSET, REFERRAL_FEE_PERCENT_OFFSET, TOKEN_A_MINT_OFFSET,
    TOKEN_B_MINT_OFFSET,
};
use crate::domain::PoolAccountProperties;
use rust_decimal::Decimal;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

const CP_AMM_ACCOUNT_LEN: usize = 1112;

/// A cp-amm `Pool` account with the discriminator, the cliff fee numerator, the
/// three fee-split percents and the two mints at their real offsets.
fn cp_amm_account(
    cliff_fee_numerator: u64,
    percents: (u8, u8, u8),
    token_a: Pubkey,
    token_b: Pubkey,
) -> Vec<u8> {
    let mut bytes = vec![0u8; CP_AMM_ACCOUNT_LEN];
    bytes[..8].copy_from_slice(&POOL_DISCRIMINATOR);
    bytes[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
        .copy_from_slice(&cliff_fee_numerator.to_le_bytes());
    bytes[PROTOCOL_FEE_PERCENT_OFFSET] = percents.0;
    bytes[PARTNER_FEE_PERCENT_OFFSET] = percents.1;
    bytes[REFERRAL_FEE_PERCENT_OFFSET] = percents.2;
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
    // 2_500_000 / 1e9 = 0.25% = 25 bps; (protocol, partner, referral) =
    // (20, 0, 20) — a real mainnet constant-fee value and split.
    let data = cp_amm_account(2_500_000, (20, 0, 20), pk(2), pk(3));

    let decoded = decode_pool_account(&cp_amm_owner(), &data).expect("should decode");

    assert_eq!(decoded.protocol(), Protocol::MeteoraDammV2);
    let PoolAccountProperties::MeteoraDammV2(props) = decoded;
    assert_eq!(props.token_a_mint, pk(2));
    assert_eq!(props.token_b_mint, pk(3));
    assert_eq!(props.fee_bps, Decimal::new(25, 0));
    assert_eq!(props.protocol_fee_percent, 20);
    assert_eq!(props.partner_fee_percent, 0);
    assert_eq!(props.referral_fee_percent, 20);
}

// ── The two guards ──────────────────────────────────────────────────

#[test]
fn unknown_owner_is_not_decoded() {
    let data = cp_amm_account(2_500_000, (20, 0, 20), pk(2), pk(3));

    assert!(decode_pool_account(&pk(99), &data).is_none());
}

#[test]
fn wrong_discriminator_is_not_decoded() {
    let mut data = cp_amm_account(2_500_000, (20, 0, 20), pk(2), pk(3));
    data[..8].fill(0);

    assert!(decode_pool_account(&cp_amm_owner(), &data).is_none());
}

#[test]
fn account_too_short_for_the_layout_is_not_decoded() {
    assert!(decode_pool_account(&cp_amm_owner(), &[0u8; 16]).is_none());
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

    // Rejected on owner…
    let dlmm_owner = Protocol::MeteoraDlmm.program_id();
    assert!(decode_pool_account(&dlmm_owner, &lb_pair).is_none());

    // …and, were it ever routed here, on the discriminator too.
    assert!(
        super::meteora::damm_v2::decode_pool_account(&lb_pair).is_none(),
        "cp-amm decoder must reject an LbPair even if handed one directly"
    );
}

// ── Protocols recognized but not decoded yet ────────────────────────

#[test]
fn known_protocol_without_a_decoder_yields_none() {
    // DAMM v1 and DLMM are valid program ids — they must not be mistaken for
    // unknown owners, but they have no pool-account decoder yet.
    for protocol in [Protocol::MeteoraDammV1, Protocol::MeteoraDlmm] {
        assert!(
            decode_pool_account(&protocol.program_id(), &[0u8; 1112]).is_none(),
            "{protocol} should decode to None"
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
