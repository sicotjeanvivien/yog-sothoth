use super::*;

#[test]
fn display_redacts_query_string() {
    let url = SecretUrl::new("https://mainnet.helius-rpc.com/?api-key=abc123");
    assert_eq!(
        format!("{url}"),
        "https://mainnet.helius-rpc.com/?***REDACTED***"
    );
}

#[test]
fn debug_redacts_query_string() {
    let url = SecretUrl::new("wss://mainnet.helius-rpc.com/?api-key=abc123");
    assert_eq!(
        format!("{url:?}"),
        "SecretUrl(wss://mainnet.helius-rpc.com/?***REDACTED***)"
    );
}

#[test]
fn expose_returns_raw_url() {
    let raw = "https://mainnet.helius-rpc.com/?api-key=abc123";
    let url = SecretUrl::new(raw);
    assert_eq!(url.expose(), raw);
}

#[test]
fn url_without_query_is_unchanged_in_display() {
    let url = SecretUrl::new("https://api.mainnet-beta.solana.com");
    assert_eq!(format!("{url}"), "https://api.mainnet-beta.solana.com");
}
