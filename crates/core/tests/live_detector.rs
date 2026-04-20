//! Live integration tests — hit the real Solana mainnet RPC.
//!
//! These tests are `#[ignore]` by default. Run with:
//!
//! ```sh
//! cargo test --package yog-core --test live_detector -- --ignored --nocapture
//! ```

use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use solana_transaction_status::UiTransactionEncoding;
use std::str::FromStr;

use yog_core::protocols::{meteora::MeteoraDammV2, PoolIndexer};

const MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";

/// Known AddLiquidity transaction on DAMM v2.
/// Pool: Meteora (RCSC-WSOL) Market — 43xBARFhhAcLJ5V7z3c26WeNLsQRdmzPyrnRyKBMDNix
const KNOWN_ADD_LIQUIDITY_SIG: &str =
    "3MEnBuodZBHDRfZkzNA9pN7bzrnYufeVbqEieFsqrcMf8VbtXL7HsAdMdE7NM2CHyaZJjtSLA82Vm1YDiiKBLq8Y";

#[tokio::test]
#[ignore]
async fn detector_recognizes_known_add_liquidity() {
    let tx = fetch_tx(KNOWN_ADD_LIQUIDITY_SIG).await;
    let indexer = MeteoraDammV2::new();

    let is_swap = indexer.is_swap(&tx);
    let is_add = indexer.is_add_liquidity(&tx);
    let is_remove = indexer.is_remove_liquidity(&tx);

    println!("--- Detector results ---");
    println!("  is_swap:           {is_swap}");
    println!("  is_add_liquidity:  {is_add}");
    println!("  is_remove_liquidity: {is_remove}");

    assert!(
        is_add,
        "detector should recognize {KNOWN_ADD_LIQUIDITY_SIG} as AddLiquidity"
    );
    assert!(!is_swap, "should NOT be flagged as swap");
    assert!(!is_remove, "should NOT be flagged as remove_liquidity");
}

#[tokio::test]
#[ignore]
async fn parser_extracts_add_liquidity_correctly() {
    let tx = fetch_tx(KNOWN_ADD_LIQUIDITY_SIG).await;
    let indexer = MeteoraDammV2::new();

    let event = indexer
        .parse_add_liquidity(&tx)
        .expect("parse_add_liquidity should succeed");

    println!("--- Parsed event ---");
    println!("  pool_address:  {}", event.pool_address);
    println!("  protocol:      {}", event.protocol);
    println!("  token_a_mint:  {}", event.token_a_mint);
    println!("  token_b_mint:  {}", event.token_b_mint);
    println!("  amount_a:      {}", event.amount_a);
    println!("  amount_b:      {}", event.amount_b);
    println!("  signature:     {}", event.signature);

    assert_eq!(
        event.pool_address.to_string(),
        "43xBARFhhAcLJ5V7z3c26WeNLsQRdmzPyrnRyKBMDNix",
        "pool should be the RCSC-WSOL Market, not the Position NFT"
    );
}

// ============================================================
// Helpers
// ============================================================

async fn fetch_tx(sig_str: &str) -> solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta {
    let rpc = RpcClient::new(MAINNET_RPC.to_string());
    let sig = Signature::from_str(sig_str).expect("valid signature");

    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };

    rpc.get_transaction_with_config(&sig, config)
        .await
        .expect("RPC fetch should succeed")
}