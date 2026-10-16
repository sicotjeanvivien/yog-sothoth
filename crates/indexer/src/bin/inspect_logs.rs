//! Spike: dumps the raw `logsNotification`s of the DAMM v2 program to stdout, to tell whether a
//! pool address can be read off the logs alone, without `getTransaction`.
//! `cargo run --bin inspect_logs > log/damm_v2_raw_logs.txt`, then Ctrl-C after 50-100 samples.

use dotenvy::dotenv;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// DAMM v2 program ID on Solana mainnet.
const DAMM_V2_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Max notifications to dump before exiting.
const MAX_NOTIFICATIONS: usize = 100;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    yog_bootstrap::init_rustls();
    dotenv().ok();
    // ⚠️ `required_endpoint`, not the indexer's `required_endpoint_allowing_header`: a bin cannot
    // reach `infra::endpoint::credential`, so a configured header pair refuses to start here.
    let stream = yog_bootstrap::required_endpoint("INGEST_STREAM")
        .context("the INGEST_STREAM endpoint is misconfigured — see .env.example")?;

    eprintln!("# Connecting to {stream}");
    let (mut ws, _) = connect_async(stream.url().expose())
        .await
        .context("failed to connect to RPC WebSocket")?;
    eprintln!("# Connected.");

    // Every log mentioning the DAMM v2 program, at the indexer's `confirmed` commitment.
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

/// Print one notification as a human-readable block on stdout.
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
