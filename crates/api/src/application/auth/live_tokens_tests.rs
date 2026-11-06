use super::*;

const NOW: i64 = 1_000;

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
    let tokens = LiveTokens::default();
    let token = tokens.issue("dashboard", &granted(), NOW).unwrap();
    (tokens, token)
}

/// Mutation: the client taken from anywhere but the token's grant.
#[test]
fn a_token_opens_its_scope_for_its_client() {
    let (tokens, token) = issued();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 1).as_deref(),
        Ok("dashboard")
    );
}

/// Mutation: the grant left in the registry, so a token opens twice.
#[test]
fn a_token_opens_once() {
    let (tokens, token) = issued();
    tokens.open(&token, &granted(), NOW + 1).unwrap();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 2),
        Err(Unauthorized::TokenUnknown)
    );
}

/// Mutation: the registry not shared between clones — the state is cloned for each request.
#[test]
fn a_clone_of_the_registry_opens_what_the_original_issued() {
    let (tokens, token) = issued();

    assert!(tokens.clone().open(&token, &granted(), NOW + 1).is_ok());
    assert_eq!(
        tokens.open(&token, &granted(), NOW + 2),
        Err(Unauthorized::TokenUnknown)
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

/// Mutation: the token taken out before its scope is checked, so a token tried elsewhere is spent.
#[test]
fn a_token_tried_on_another_path_still_opens_its_own() {
    let (tokens, token) = issued();
    let _ = tokens.open(&token, &scope("/api/live", &["signals"]), NOW + 1);

    assert!(tokens.open(&token, &granted(), NOW + 2).is_ok());
}

/// Mutation: the purge removed, so the registry keeps every token ever issued.
#[test]
fn an_expired_token_is_forgotten_at_the_next_issue() {
    let (tokens, token) = issued();
    tokens.issue("dashboard", &granted(), NOW + 40).unwrap();

    assert_eq!(
        tokens.open(&token, &granted(), NOW + 41),
        Err(Unauthorized::TokenUnknown),
        "forgotten, not merely expired"
    );
}

#[test]
fn a_token_never_issued_is_unknown() {
    let (tokens, _) = issued();

    assert_eq!(
        tokens.open(&format!("ylt_{}", "0".repeat(64)), &granted(), NOW + 1),
        Err(Unauthorized::TokenUnknown)
    );
}

/// Mutation: any bearer looked up, so an API key or a JWT reads as a token never issued.
#[test]
fn a_bearer_of_another_form_is_malformed() {
    let (tokens, token) = issued();

    for bearer in [
        token.replacen("ylt_", "yog_", 1),
        token[..token.len() - 2].to_string(),
        token.to_uppercase().replacen("YLT_", "ylt_", 1),
        "eyJhbGciOiJIUzI1NiJ9.e30.x".to_string(),
    ] {
        assert_eq!(
            tokens.open(&bearer, &granted(), NOW + 1),
            Err(Unauthorized::TokenMalformed),
            "{bearer}"
        );
    }
}

/// Mutation: one cap for every client, so a flood under one key cuts the live stream of all;
/// or a cap freed at the opening, so issue-and-open in a loop grows the expiry queue.
#[test]
fn past_its_cap_a_client_is_refused_and_the_others_are_not() {
    let tokens = LiveTokens::default();
    for _ in 0..MAX_LIVE_TOKENS_PER_CLIENT {
        let token = tokens.issue("flood", &granted(), NOW).unwrap();
        tokens.open(&token, &granted(), NOW).unwrap();
    }

    assert!(matches!(
        tokens.issue("flood", &granted(), NOW + 1),
        Err(IssueError::Full)
    ));
    assert!(tokens.issue("dashboard", &granted(), NOW + 1).is_ok());
    assert!(
        tokens.issue("flood", &granted(), NOW + 30).is_ok(),
        "its first tokens expired"
    );
}
