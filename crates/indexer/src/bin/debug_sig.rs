//! Debug tool — fetch a transaction by signature and run it through the
//! DAMM v2 parser directly, bypassing the WebSocket pipeline entirely.
//!
//! Usage:
//!   cargo run --bin debug_sig -- <SIGNATURE>
//!   cargo run --bin debug_sig -- <SIGNATURE> --dump path/to/fixture.json
//!
//! The `--dump` flag writes the raw transaction JSON to disk so you can
//! turn a failing case into a regression test fixture.
//!
//! Placement: this file lives at `crates/indexer/src/bin/debug_sig.rs`
//! and is picked up automatically by Cargo as a binary target.

use std::{env, str::FromStr};

use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{
    config::RpcTransactionConfig, response::transaction::Signature,
};
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta,
    UiTransactionEncoding,
};
use tracing::info;
use tracing_subscriber::EnvFilter;
use yog_core::{
    domain::Protocol,
    protocols::{
        meteora::{MeteoraDammV1, MeteoraDammV2, MeteoraDlmm},
        PoolIndexer,
    },
};

const DEFAULT_RPC: &str = "https://api.mainnet-beta.solana.com";

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,yog_core=debug")),
        )
        .init();

    let (signature_str, dump_path) = parse_args()?;
    let signature = Signature::from_str(&signature_str)
        .map_err(|e| anyhow::anyhow!("invalid signature: {e}"))?;

    let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC.to_string());
    info!(rpc = %rpc_url, %signature, "fetching transaction");

    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let tx = fetch_tx(&client, &signature).await?;

    if let Some(path) = dump_path.as_deref() {
        dump_fixture(&tx, path)?;
    }

    summarize_logs(&tx);
    let protocol = match detect_protocol(&tx) {
        Some(p) => p,
        None => {
            println!("\n❌ Aucun program ID de protocole connu trouvé dans les logs.");
            println!(
                "   Program IDs surveillés : DAMM v2 / DAMM v1 / DLMM. La tx mentionne peut-être"
            );
            println!("   ces programmes via un CPI plus profond que ce que `logs` remonte.");
            return Ok(());
        }
    };

    println!("\n🔎 Protocole détecté : {}", protocol.as_str());
    probe_parser(protocol, &tx);
    Ok(())
}

/// Parse CLI arguments: `<signature> [--dump <path>]`.
fn parse_args() -> anyhow::Result<(String, Option<String>)> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        anyhow::bail!("usage: debug_sig <SIGNATURE> [--dump <path>]");
    }
    let signature = args[0].clone();
    let mut dump = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dump" => {
                let path = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--dump requires a path"))?
                    .clone();
                dump = Some(path);
                i += 2;
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok((signature, dump))
}

/// Fetch a confirmed transaction by signature, no retries — this is a debug tool.
async fn fetch_tx(
    client: &RpcClient,
    signature: &Signature,
) -> anyhow::Result<EncodedConfirmedTransactionWithStatusMeta> {
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };
    client
        .get_transaction_with_config(signature, config)
        .await
        .map_err(|e| anyhow::anyhow!("get_transaction failed: {e}"))
}

/// Write the raw transaction JSON to disk for fixture replay.
fn dump_fixture(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    path: &str,
) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(tx)?;
    std::fs::write(path, json)?;
    println!("💾 Fixture écrite : {path}");
    Ok(())
}

/// Print every log message from the tx, numbered. Helpful to eyeball whether
/// `Program log: Instruction: <Name>` is actually present.
fn summarize_logs(tx: &EncodedConfirmedTransactionWithStatusMeta) {
    let Some(meta) = tx.transaction.meta.as_ref() else {
        println!("⚠️  Pas de meta dans la tx.");
        return;
    };
    println!("\n📜 Logs de la transaction :");
    match &meta.log_messages {
        OptionSerializer::Some(logs) => {
            for (i, log) in logs.iter().enumerate() {
                println!("  [{i:02}] {log}");
            }
        }
        _ => println!("  (aucun log disponible)"),
    }
    if let Some(err) = &meta.err {
        println!("\n⚠️  La tx a échoué on-chain : {err:?}");
    }
}

/// Detect which Meteora protocol this tx touches by scanning the logs for
/// a known program ID invoke marker.
fn detect_protocol(tx: &EncodedConfirmedTransactionWithStatusMeta) -> Option<Protocol> {
    let meta = tx.transaction.meta.as_ref()?;
    let OptionSerializer::Some(logs) = &meta.log_messages else {
        return None;
    };
    let candidates = [
        Protocol::MeteoraDammV2,
        Protocol::MeteoraDammV1,
        Protocol::MeteoraDlmm,
    ];
    for protocol in candidates {
        let marker = format!("Program {} invoke", protocol.program_id());
        if logs.iter().any(|log| log.starts_with(&marker)) {
            return Some(protocol);
        }
    }
    None
}

/// Run the three detectors and try to parse whatever matches. Prints the full
/// parsed event on success, or the parser error on failure.
fn probe_parser(protocol: Protocol, tx: &EncodedConfirmedTransactionWithStatusMeta) {
    let indexer: Box<dyn PoolIndexer> = match protocol {
        Protocol::MeteoraDammV2 => Box::new(MeteoraDammV2::new()),
        Protocol::MeteoraDammV1 => Box::new(MeteoraDammV1::new()),
        Protocol::MeteoraDlmm => Box::new(MeteoraDlmm::new()),
    };

    let is_swap = indexer.is_swap(tx);
    let is_add = indexer.is_add_liquidity(tx);
    let is_remove = indexer.is_remove_liquidity(tx);

    println!("\n🎯 Détecteurs :");
    println!("   is_swap            = {is_swap}");
    println!("   is_add_liquidity   = {is_add}");
    println!("   is_remove_liquidity= {is_remove}");

    if !(is_swap || is_add || is_remove) {
        println!("\n❌ Aucun détecteur ne matche. Pistes :");
        println!("   • vérifier que la tx n'est pas échouée (meta.err)");
        println!("   • vérifier que les logs contiennent bien `Program log: Instruction: ...`");
        println!("   • si la tx passe en CPI depuis un autre programme, vérifier que");
        println!("     l'ordre des logs est compatible avec la machine d'état de `is_instruction`");
        return;
    }

    if is_swap {
        println!("\n🔄 Tentative parse_swap ...");
        match indexer.parse_swap(tx) {
            Ok(ev) => println!("✅ SwapEvent = {ev:#?}"),
            Err(e) => println!("❌ parse_swap a échoué : {e}"),
        }
    }
    if is_add {
        println!("\n➕ Tentative parse_add_liquidity ...");
        match indexer.parse_add_liquidity(tx) {
            Ok(ev) => println!("✅ LiquidityEvent (Add) = {ev:#?}"),
            Err(e) => println!("❌ parse_add_liquidity a échoué : {e}"),
        }
    }
    if is_remove {
        println!("\n➖ Tentative parse_remove_liquidity ...");
        match indexer.parse_remove_liquidity(tx) {
            Ok(ev) => println!("✅ LiquidityEvent (Remove) = {ev:#?}"),
            Err(e) => println!("❌ parse_remove_liquidity a échoué : {e}"),
        }
    }
}