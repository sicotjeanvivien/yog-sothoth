//! Unit tests for the cp-amm pool account decoder.
//!
//! No network: build a synthetic account buffer matching the cp-amm
//! `Pool` layout and assert the mints come out of the right offsets.

use super::*;
use base64::Engine;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Build a base64 account `data.0` with the discriminator and the two
/// mints placed at their real offsets.
fn account_data(token_a: Pubkey, token_b: Pubkey) -> String {
    let mut bytes = vec![0u8; 1112];
    bytes[..8].copy_from_slice(&POOL_DISCRIMINATOR);
    bytes[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32].copy_from_slice(token_a.as_ref());
    bytes[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32].copy_from_slice(token_b.as_ref());
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn client() -> CpAmmPoolClient {
    CpAmmPoolClient::new("http://localhost".to_string())
}

#[test]
fn decodes_mints_at_correct_offsets() {
    let c = client();
    let account = RpcAccount {
        data: (account_data(pk(2), pk(3)), "base64".to_string()),
        owner: Protocol::MeteoraDammV2.program_id().to_string(),
    };
    let resolved = c.decode(pk(1), account).expect("should decode");
    assert_eq!(resolved.pool, pk(1));
    assert_eq!(resolved.token_a_mint, pk(2));
    assert_eq!(resolved.token_b_mint, pk(3));
}

#[test]
fn rejects_wrong_owner() {
    let c = client();
    let account = RpcAccount {
        data: (account_data(pk(2), pk(3)), "base64".to_string()),
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
