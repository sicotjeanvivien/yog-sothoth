//! `getMultipleAccounts` against a local server: what is asked, and whose account comes back.

use serde_json::{Value, json};

use super::super::scripted_provider::ScriptedProvider;
use super::*;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn account(seed: u8) -> Value {
    json!({
        "data": [base64::engine::general_purpose::STANDARD.encode([seed; 4]), "base64"],
        "owner": pk(seed + 100).to_string(),
    })
}

fn answer(value: Vec<Value>) -> (u16, Value) {
    (200, json!({ "result": { "value": value } }))
}

fn pools(accounts: &[RawAccount]) -> Vec<Pubkey> {
    accounts.iter().map(|a| a.pool_address).collect()
}

/// Mutation: the absent accounts filtered before the addresses are matched, or the addresses read
/// in another order: an account, and its mints, would then belong to the wrong pool.
#[tokio::test]
async fn each_account_comes_back_to_its_address_around_an_absent_one() {
    let provider =
        ScriptedProvider::answering(vec![answer(vec![account(1), Value::Null, account(3)])]).await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let accounts = client
        .fetch_accounts(&[pk(1), pk(2), pk(3)])
        .await
        .expect("the accounts");

    assert_eq!(pools(&accounts), [pk(1), pk(3)]);
    assert_eq!(accounts[0].data, [1; 4]);
    assert_eq!(accounts[0].program_id, pk(101));
    assert_eq!(accounts[1].data, [3; 4]);
    assert_eq!(accounts[1].program_id, pk(103));
}

/// Mutation: another method or encoding asked, or the addresses not sent.
#[tokio::test]
async fn the_request_asks_for_the_addresses_in_base64() {
    let provider = ScriptedProvider::answering(vec![answer(vec![account(1)])]).await;
    let client = SolanaAccountClient::new(provider.url.clone());

    client.fetch_accounts(&[pk(1)]).await.expect("the accounts");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "getMultipleAccounts");
    assert_eq!(requests[0]["params"][0], json!([pk(1).to_string()]));
    assert_eq!(requests[0]["params"][1]["encoding"], "base64");
}

/// Mutation: an unreadable owner or content failing the whole batch.
#[tokio::test]
async fn an_unreadable_account_is_dropped_alone() {
    let provider = ScriptedProvider::answering(vec![answer(vec![
        json!({ "data": ["AAAA", "base64"], "owner": "not a key" }),
        json!({ "data": ["not base64!", "base64"], "owner": pk(9).to_string() }),
        account(3),
    ])])
    .await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let accounts = client
        .fetch_accounts(&[pk(1), pk(2), pk(3)])
        .await
        .expect("the batch survives");

    assert_eq!(pools(&accounts), [pk(3)]);
}

/// Mutation: the batch size raised, which the RPC refuses, or a batch lost.
#[tokio::test]
async fn past_a_hundred_addresses_every_batch_is_asked() {
    let addresses: Vec<Pubkey> = (0..=100).map(pk).collect();
    let provider = ScriptedProvider::answering(vec![
        answer((0..100).map(account).collect()),
        answer(vec![account(100)]),
    ])
    .await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let accounts = client
        .fetch_accounts(&addresses)
        .await
        .expect("the accounts");

    assert_eq!(pools(&accounts), addresses);
    let sizes: Vec<usize> = provider
        .requests()
        .iter()
        .map(|request| request["params"][0].as_array().expect("addresses").len())
        .collect();
    assert_eq!(sizes, [100, 1]);
}

/// Mutation: `error_for_status` removed, or the URL, which holds the key, kept in the error. These
/// clients have no rate-limit type: a 429 is `Http`.
#[tokio::test]
async fn a_refused_call_is_an_http_error() {
    for status in [500, 429] {
        let provider = ScriptedProvider::answering(vec![(status, json!({}))]).await;
        let client = SolanaAccountClient::new(provider.url.clone());

        let refused = client.fetch_accounts(&[pk(1)]).await;

        assert!(
            matches!(refused, Err(SourceError::Http(_))),
            "{status}: {refused:?}"
        );
        assert_eq!(
            provider.requests().len(),
            1,
            "the request reached the server"
        );
        let error = refused.expect_err("a refusal").to_string();
        assert!(!error.contains("secret"), "{error}");
    }
}

/// Mutation: a missing `result` read as no account: an RPC error comes back as a 200.
#[tokio::test]
async fn an_rpc_error_is_a_decode_error() {
    let body = json!({ "error": { "code": -32005, "message": "too many requests" } });
    let provider = ScriptedProvider::answering(vec![(200, body)]).await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let refused = client.fetch_accounts(&[pk(1)]).await;

    assert!(
        matches!(refused, Err(SourceError::Decode(_))),
        "{refused:?}"
    );
}

/// Pinned as it is: unlike Helius, a failed batch fails the whole call.
/// Mutation: a failed batch skipped.
#[tokio::test]
async fn a_failed_batch_fails_the_whole_call() {
    let addresses: Vec<Pubkey> = (0..=100).map(pk).collect();
    let provider = ScriptedProvider::answering(vec![
        answer((0..100).map(account).collect()),
        (500, json!({})),
    ])
    .await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let failed = client.fetch_accounts(&addresses).await;

    assert!(matches!(failed, Err(SourceError::Http(_))), "{failed:?}");
    assert_eq!(
        provider.requests().len(),
        2,
        "both requests reached the server"
    );
}

/// Pinned as it is. Mutation: the answers matched to the addresses from the end.
#[tokio::test]
async fn a_short_answer_keeps_the_accounts_it_has() {
    let provider = ScriptedProvider::answering(vec![answer(vec![account(1)])]).await;
    let client = SolanaAccountClient::new(provider.url.clone());

    let accounts = client
        .fetch_accounts(&[pk(1), pk(2), pk(3)])
        .await
        .expect("the accounts");

    assert_eq!(pools(&accounts), [pk(1)]);
}
