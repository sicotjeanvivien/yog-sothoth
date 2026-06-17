//! Unit tests for the cp-amm pool account decoder.
//!
//! No network: build a synthetic account buffer matching the cp-amm
//! `Pool` layout and assert the mints + base fee come out of the right
//! offsets.

use super::*;
use base64::Engine;
use rust_decimal::Decimal;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Build a base64 account `data.0` with the discriminator, the cliff fee
/// numerator, the three fee-split percents and the two mints placed at their
/// real offsets.
fn account_data(
    cliff_fee_numerator: u64,
    percents: (u8, u8, u8),
    token_a: Pubkey,
    token_b: Pubkey,
) -> String {
    let mut bytes = vec![0u8; 1112];
    bytes[..8].copy_from_slice(&POOL_DISCRIMINATOR);
    bytes[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
        .copy_from_slice(&cliff_fee_numerator.to_le_bytes());
    bytes[PROTOCOL_FEE_PERCENT_OFFSET] = percents.0;
    bytes[PARTNER_FEE_PERCENT_OFFSET] = percents.1;
    bytes[REFERRAL_FEE_PERCENT_OFFSET] = percents.2;
    bytes[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32].copy_from_slice(token_a.as_ref());
    bytes[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32].copy_from_slice(token_b.as_ref());
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn client() -> CpAmmPoolClient {
    CpAmmPoolClient::new("http://localhost".to_string())
}

#[test]
fn decodes_mints_and_fee_at_correct_offsets() {
    let c = client();
    let account = RpcAccount {
        // 2_500_000 / 1e9 = 0.25% = 25 bps; (protocol, partner, referral) =
        // (20, 0, 20) — a real mainnet constant-fee value and split.
        data: (
            account_data(2_500_000, (20, 0, 20), pk(2), pk(3)),
            "base64".to_string(),
        ),
        owner: Protocol::MeteoraDammV2.program_id().to_string(),
    };
    let resolved = c.decode(pk(1), account).expect("should decode");
    assert_eq!(resolved.pool, pk(1));
    assert_eq!(resolved.properties.token_a_mint, pk(2));
    assert_eq!(resolved.properties.token_b_mint, pk(3));
    assert_eq!(resolved.properties.fee_bps, Decimal::new(25, 0));
    assert_eq!(resolved.properties.protocol_fee_percent, 20);
    assert_eq!(resolved.properties.partner_fee_percent, 0);
    assert_eq!(resolved.properties.referral_fee_percent, 20);
}

#[test]
fn rejects_wrong_owner() {
    let c = client();
    let account = RpcAccount {
        data: (
            account_data(2_500_000, (20, 0, 20), pk(2), pk(3)),
            "base64".to_string(),
        ),
        owner: pk(99).to_string(), // not the cp-amm program
    };
    assert!(c.decode(pk(1), account).is_none());
}

#[test]
fn rejects_bad_discriminator() {
    let c = client();
    let mut bytes = vec![0u8; 1112];
    // Leave the discriminator zeroed (≠ POOL_DISCRIMINATOR).
    bytes[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32].copy_from_slice(pk(2).as_ref());
    let account = RpcAccount {
        data: (
            base64::engine::general_purpose::STANDARD.encode(bytes),
            "base64".to_string(),
        ),
        owner: Protocol::MeteoraDammV2.program_id().to_string(),
    };
    assert!(c.decode(pk(1), account).is_none());
}

#[test]
fn rejects_short_account() {
    let c = client();
    let account = RpcAccount {
        data: (
            base64::engine::general_purpose::STANDARD.encode([0u8; 16]),
            "base64".to_string(),
        ),
        owner: Protocol::MeteoraDammV2.program_id().to_string(),
    };
    assert!(c.decode(pk(1), account).is_none());
}
