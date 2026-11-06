use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::*;

const NOW: i64 = 1_000;

fn tokens() -> LiveTokens {
    LiveTokens::new(b"a secret of the api, long enough to sign")
}

fn scope(path: &str, topics: &[&str]) -> LiveScope {
    LiveScope {
        path: path.to_string(),
        topics: topics.iter().map(|t| t.to_string()).collect(),
    }
}

fn granted() -> LiveScope {
    scope(
        "/api/live/protocols/meteora_damm_v2",
        &["signals", "pool-creations"],
    )
}

fn issued() -> (LiveTokens, String) {
    let tokens = tokens();
    let token = tokens.issue("dashboard", &granted(), NOW).unwrap();
    (tokens, token)
}

/// The token with its payload replaced by `claims`, signed with `tokens`' key.
fn resigned(tokens: &LiveTokens, claims: &str) -> String {
    let signed = format!("{}.{}", tokens.header, URL_SAFE_NO_PAD.encode(claims));
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(tokens.sign(&signed)))
}

/// Mutation: the client taken from anywhere but the token.
#[test]
fn a_token_opens_its_scope_for_its_client() {
    let (tokens, token) = issued();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 1).as_deref(),
        Ok("dashboard")
    );
}

/// Mutation: the id recorded and never checked, so a token opens twice.
#[test]
fn a_token_opens_once() {
    let (tokens, token) = issued();
    tokens.open(&token, &granted(), NOW + 1).unwrap();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 2),
        Err(Unauthorized::TokenUsed)
    );
}

/// Mutation: the expiry compared with `>`, so a token opens at its last second.
#[test]
fn a_token_expires_after_thirty_seconds() {
    let (tokens, token) = issued();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 30),
        Err(Unauthorized::TokenExpired)
    );
}

#[test]
fn a_token_does_not_open_another_path() {
    let (tokens, token) = issued();

    assert_eq!(
        tokens.open(&token, &scope("/api/live", &["signals"]), NOW + 1),
        Err(Unauthorized::TokenScope)
    );
}

#[test]
fn a_token_does_not_open_a_topic_of_its_own() {
    let (tokens, token) = issued();
    let wider = scope(
        "/api/live/protocols/meteora_damm_v2",
        &["signals", "pool-creations", "liquidity-changes"],
    );

    assert_eq!(
        tokens.open(&token, &wider, NOW + 1),
        Err(Unauthorized::TokenScope)
    );
}

/// Mutation: the scope checked after the id is consumed, so a token tried elsewhere is spent.
#[test]
fn a_token_tried_on_another_path_still_opens_its_own() {
    let (tokens, token) = issued();
    let _ = tokens.open(&token, &scope("/api/live", &["signals"]), NOW + 1);

    assert!(tokens.open(&token, &granted(), NOW + 2).is_ok());
}

/// Mutation: the signature left unchecked, so a payload rewritten by the bearer opens.
#[test]
fn a_rewritten_payload_breaks_the_signature() {
    let (tokens, token) = issued();
    let mut parts: Vec<&str> = token.split('.').collect();
    let wider = URL_SAFE_NO_PAD.encode(
        r#"{"sub":"dashboard","path":"/api/live","topics":["signals"],"jti":"00","iat":1000,"exp":9999}"#,
    );
    parts[1] = &wider;

    assert_eq!(
        tokens.open(&parts.join("."), &scope("/api/live", &["signals"]), NOW + 1),
        Err(Unauthorized::TokenSignature)
    );
}

#[test]
fn a_token_signed_with_another_key_is_refused() {
    let token = LiveTokens::new(b"another key entirely, also long")
        .issue("dashboard", &granted(), NOW)
        .unwrap();

    assert_eq!(
        tokens().open(&token, &granted(), NOW + 1),
        Err(Unauthorized::TokenSignature)
    );
}

/// Mutation: the header read for its `alg`, so `none` passes without a signature.
#[test]
fn a_token_under_another_header_is_malformed() {
    let none = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let (tokens, token) = issued();
    let rest = token.split_once('.').unwrap().1;

    assert_eq!(
        tokens.open(&format!("{none}.{rest}"), &granted(), NOW + 1),
        Err(Unauthorized::TokenMalformed)
    );
}

#[test]
fn a_bearer_of_two_or_four_parts_is_malformed() {
    let (tokens, token) = issued();

    assert_eq!(
        tokens.open("a.b", &granted(), NOW + 1),
        Err(Unauthorized::TokenMalformed)
    );
    assert_eq!(
        tokens.open(&format!("{token}.x"), &granted(), NOW + 1),
        Err(Unauthorized::TokenMalformed)
    );
}

#[test]
fn a_signed_payload_that_is_no_claims_is_malformed() {
    let tokens = tokens();

    assert_eq!(
        tokens.open(&resigned(&tokens, r#"{"sub":"x"}"#), &granted(), NOW + 1),
        Err(Unauthorized::TokenMalformed)
    );
}
