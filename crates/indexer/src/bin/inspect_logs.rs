//! Throwaway spike — dumps raw WebSocket logs from the DAMM v2 program.
//!
//! Purpose: determine whether the pool address is extractable from the
//! `logsNotification` messages alone, without fetching the full transaction
//! via `getTransaction`. This is the prerequisite for `WatchedPoolFilter`
//! to filter BEFORE the HTTP fetch (Option 1) rather than after parsing
//! (Option 2).
//!
//! Output is one YAML-like block per notification on stdout. Redirect to
//! a file to inspect at leisure:
//!
//!     cargo run --bin inspect_logs > log/damm_v2_raw_logs.txt
//!
//! Stop with Ctrl-C after collecting enough samples (50-100 is plenty).
//! Then grep the captured file for pool addresses (known from fixtures
//! or from the watched_pools table) to see if and where they appear.
//!
//! Delete this file once the question is answered.

use dotenvy::dotenv;
use std::env;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// DAMM v2 program ID on Solana mainnet.
const DAMM_V2_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Max notifications to dump before exiting. Adjust as needed.
const MAX_NOTIFICATIONS: usize = 100;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");
    dotenv().ok();
    // Read WS URL from env — same key as the main indexer for consistency.
    let ws_url = env::var("SOLANA_RPC_WS")
        .context("SOLANA_RPC_WS must be set (e.g. wss://api.mainnet-beta.solana.com)")?;

    eprintln!("# Connecting to {}", redact(&ws_url));
    let (mut ws, _) = connect_async(&ws_url)
        .await
        .context("failed to connect to RPC WebSocket")?;
    eprintln!("# Connected.");

    // Subscribe to every log where the DAMM v2 program is mentioned.
    // `commitment: confirmed` matches the main indexer — same selection criteria.
    let subscribe = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "logsSubscribe",
        "params": [
            { "mentions": [DAMM_V2_PROGRAM_ID] },
            { "commitment": "confirmed" }
        ]
    });

    ws.send(Message::Text(subscribe.to_string().into()))
        .await
        .context("failed to send logsSubscribe")?;

    // First message is the subscription ack — display it and move on.
    let ack = timeout(Duration::from_secs(10), ws.next())
        .await
        .context("timed out waiting for subscription ack")?
        .ok_or_else(|| anyhow!("connection closed before ack"))?
        .context("ws error while reading ack")?;
    eprintln!("# Subscription ack: {}\n", ack);

    let mut count: usize = 0;
    while count < MAX_NOTIFICATIONS {
        let msg = match ws.next().await {
            Some(Ok(m)) => m,
            Some(Err(e)) => {
                eprintln!("# ws error: {e}");
                break;
            }
            None => {
                eprintln!("# ws closed by peer");
                break;
            }
        };

        // Only text frames carry notifications. Ignore ping/pong/binary.
        let text = match msg {
            Message::Text(t) => t,
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p)).await;
                continue;
            }
            _ => continue,
        };

        let parsed: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("# unparseable frame: {e} (raw: {text})");
                continue;
            }
        };

        // Extract the notification payload. Anything else (e.g. late ack,
        // unexpected RPC error) is dumped raw for inspection.
        let value = parsed.pointer("/params/result/value").cloned();

        let Some(value) = value else {
            eprintln!("# non-notification frame: {parsed}");
            continue;
        };

        count += 1;
        dump_notification(count, &value);
    }

    eprintln!("\n# Collected {count} notifications. Done.");
    Ok(())
}

/// Print one notification as a human-readable block. stdout only —
/// easy to redirect to a file for later inspection.
fn dump_notification(index: usize, value: &Value) {
    let signature = value
        .get("signature")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let err = value.get("err");
    let logs = value
        .get("logs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    println!("--- notification #{index} ---");
    println!("signature: {signature}");
    println!(
        "err: {}",
        err.map(Value::to_string)
            .unwrap_or_else(|| "null".to_string())
    );
    println!("logs ({} lines):", logs.len());
    for (i, line) in logs.iter().enumerate() {
        if let Some(s) = line.as_str() {
            println!("  [{i:>3}] {s}");
        } else {
            println!("  [{i:>3}] <non-string log entry: {line}>");
        }
    }
    println!();
}

/// Redact query-string credentials from a WS URL before logging it to stderr.
/// Helius and similar providers encode the API key in the URL path or query.
fn redact(url: &str) -> String {
    if let Some(q) = url.find('?') {
        format!("{}?…redacted…", &url[..q])
    } else if let Some(last_slash) = url.rfind('/') {
        // Helius uses `wss://mainnet.helius-rpc.com/?api-key=…` OR
        // `wss://…/v1/?api-key=…` — the `?` branch covers both. For URLs
        // without query params, redact the trailing path segment which
        // some providers use for the key.
        let (head, tail) = url.split_at(last_slash + 1);
        if tail.len() > 8 && !tail.contains('.') {
            format!("{head}…redacted…")
        } else {
            url.to_string()
        }
    } else {
        url.to_string()
    }
}
