use super::*;

// ---------------------------------------------------------------------------
// SecretUrl — the query string
// ---------------------------------------------------------------------------

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

/// A URL that carries no secret at all is left alone — and that is correct
/// *for a URL*, whose host and path are what a startup failure has to name.
///
/// ⚠️ Do not read this as a general property of the module. The same
/// pass-through applied to a bare API key is a leak, which is why [`SecretKey`]
/// has no counterpart to this test and never will: it redacts unconditionally.
/// `JUPITER_API_KEY` sat in a `SecretUrl` for months precisely because this
/// behaviour looked like a blessed one.
#[test]
fn url_carrying_no_secret_is_unchanged() {
    let url = SecretUrl::new("https://api.mainnet-beta.solana.com");
    assert_eq!(format!("{url}"), "https://api.mainnet-beta.solana.com");
}

// ---------------------------------------------------------------------------
// SecretUrl — the password in the userinfo
// ---------------------------------------------------------------------------

/// The case the module was blind to: a Postgres URL has no `?`, so the old
/// rule redacted nothing at all.
#[test]
fn display_redacts_postgres_password() {
    let url = SecretUrl::new("postgresql://yog_indexer:hunter2@localhost:5433/yog_sothoth");
    assert_eq!(
        format!("{url}"),
        "postgresql://yog_indexer:***REDACTED***@localhost:5433/yog_sothoth"
    );
}

/// The carrier is what makes this worth keeping over a blanket `****`: role,
/// host, port and database all survive, so a daemon that cannot connect still
/// says which database it failed against.
#[test]
fn postgres_redaction_keeps_the_diagnostic() {
    let url = SecretUrl::new("postgresql://yog_indexer:hunter2@localhost:5433/yog_sothoth");
    let shown = format!("{url}");
    assert!(!shown.contains("hunter2"), "password leaked: {shown}");
    for kept in ["yog_indexer", "localhost", "5433", "yog_sothoth"] {
        assert!(
            shown.contains(kept),
            "`{kept}` should stay visible: {shown}"
        );
    }
}

#[test]
fn debug_redacts_postgres_password() {
    let url = SecretUrl::new("postgresql://yog_api:hunter2@db:5432/yog_sothoth");
    assert_eq!(
        format!("{url:?}"),
        "SecretUrl(postgresql://yog_api:***REDACTED***@db:5432/yog_sothoth)"
    );
}

/// A password may contain `@`; a hostname may not. Hence `rfind` — splitting on
/// the *first* `@` would leave the tail of the password in the open.
#[test]
fn password_containing_at_sign_is_fully_redacted() {
    let url = SecretUrl::new("postgresql://yog:p@ss@localhost:5433/yog_sothoth");
    let shown = format!("{url}");
    assert_eq!(
        shown,
        "postgresql://yog:***REDACTED***@localhost:5433/yog_sothoth"
    );
    assert!(!shown.contains("p@ss"), "password leaked: {shown}");
}

/// Both secrets in one URL, both gone.
#[test]
fn password_and_query_are_both_redacted() {
    let url = SecretUrl::new("postgresql://yog:hunter2@localhost:5433/db?sslmode=require");
    assert_eq!(
        format!("{url}"),
        "postgresql://yog:***REDACTED***@localhost:5433/db?***REDACTED***"
    );
}

/// A username with no password is not a secret, and stays legible.
#[test]
fn userinfo_without_password_is_unchanged() {
    let url = SecretUrl::new("postgresql://yog@localhost:5433/yog_sothoth");
    assert_eq!(
        format!("{url}"),
        "postgresql://yog@localhost:5433/yog_sothoth"
    );
}

/// No scheme separator: nothing can be located, and nothing is invented.
#[test]
fn value_without_scheme_is_left_alone_by_the_password_rule() {
    let url = SecretUrl::new("localhost:5433/yog_sothoth");
    assert_eq!(format!("{url}"), "localhost:5433/yog_sothoth");
}

// ---------------------------------------------------------------------------
// SecretUrl — the ambiguous cases, where the rule fails closed
// ---------------------------------------------------------------------------

/// An unencoded delimiter inside the password pushes the `@` past the authority
/// bound, and nothing short of a parser can tell that from a path that happens
/// to contain an `@`. Every one of these leaked at some point: `/` and `#`
/// returned the URL untouched, `?` printed the password's prefix, and `@`
/// combined with `/` printed its tail. The rule now keeps the scheme and drops
/// the rest.
#[test]
fn ambiguous_userinfo_is_redacted_whole() {
    let cases = [
        ("postgresql://yog:pa/ss@localhost:5433/yog_sothoth", "pa/ss"),
        ("postgresql://yog:pa#ss@localhost:5433/yog_sothoth", "pa#ss"),
        ("postgresql://yog:pa?ss@localhost:5433/yog_sothoth", "pa?ss"),
        ("postgresql://yog:p@s/s@localhost:5433/yog_sothoth", "p@s/s"),
    ];
    for (raw, secret) in cases {
        let shown = format!("{}", SecretUrl::new(raw));
        assert_eq!(shown, "postgresql://***REDACTED***", "for {raw}");
        for fragment in [secret, "pa", "s/s"] {
            assert!(
                !shown.contains(fragment),
                "`{fragment}` survived in {shown} (from {raw})"
            );
        }
    }
}

/// The cost of failing closed, asserted rather than discovered: a URL with a
/// port and an `@` in its path is indistinguishable from `user:pass@host`, so
/// its host goes too. Nothing leaks; a diagnostic is lost. That trade is the
/// decision, and this test is where it is written down.
#[test]
fn failing_closed_can_cost_a_host_that_hid_nothing() {
    let url = SecretUrl::new("https://host:8080/pa@th");
    assert_eq!(format!("{url}"), "https://***REDACTED***");
}

/// The fallback is narrow: without a `:` in the authority there is no password
/// to protect, so an `@` in the path costs nothing and the host stays.
#[test]
fn an_at_in_the_path_alone_does_not_trigger_the_fallback() {
    let url = SecretUrl::new("https://api.jup.ag/price/v3@latest");
    let shown = format!("{url}");
    assert!(
        shown.starts_with("https://api.jup.ag/"),
        "host lost for nothing: {shown}"
    );
}

// ---------------------------------------------------------------------------
// SecretUrl — the credential in the path
// ---------------------------------------------------------------------------

/// Alchemy and QuickNode put the key in a path segment, and the repository's
/// own provider study puts Alchemy first. A rule that only knew about `?` would
/// print these whole — which is the defect this module exists to close, applied
/// one level up.
#[test]
fn credential_in_the_path_is_redacted() {
    for raw in [
        "https://solana-mainnet.g.alchemy.com/v2/SUPERSECRETKEY",
        "wss://solana-mainnet.g.alchemy.com/v2/SUPERSECRETKEY",
        "https://xxx.solana-mainnet.quiknode.pro/abcdef123456/",
    ] {
        let shown = format!("{}", SecretUrl::new(raw));
        assert!(
            !shown.contains("SUPERSECRETKEY") && !shown.contains("abcdef123456"),
            "credential leaked from the path: {shown}"
        );
        assert!(
            shown.contains("***REDACTED***"),
            "nothing redacted: {shown}"
        );
    }
}

/// The host names the provider, and that is the diagnostic worth keeping.
#[test]
fn path_redaction_keeps_scheme_and_host() {
    let url = SecretUrl::new("https://solana-mainnet.g.alchemy.com/v2/SUPERSECRETKEY");
    assert_eq!(
        format!("{url}"),
        "https://solana-mainnet.g.alchemy.com/***REDACTED***"
    );
}

/// A path made only of separators hides nothing — the far more common
/// "key in the query string" endpoint keeps its shape.
#[test]
fn empty_path_is_left_alone() {
    let url = SecretUrl::new("https://mainnet.helius-rpc.com/?api-key=abc123");
    assert_eq!(
        format!("{url}"),
        "https://mainnet.helius-rpc.com/?***REDACTED***"
    );
}

/// Postgres is the one scheme whose path survives: it is the database name,
/// and naming the database is what makes this type worth more than `****`.
#[test]
fn postgres_path_is_the_database_name_and_survives() {
    let url = SecretUrl::new("postgresql://yog_indexer:hunter2@localhost:5433/yog_sothoth");
    let shown = format!("{url}");
    assert!(
        shown.ends_with("/yog_sothoth"),
        "database name lost: {shown}"
    );
    assert!(!shown.contains("hunter2"), "password leaked: {shown}");
}

/// An unknown scheme gets its path hidden rather than shown. The rule fails
/// closed, so the provider nobody has met yet is covered by default.
#[test]
fn unknown_scheme_has_its_path_redacted() {
    let url = SecretUrl::new("grpc://some-new-provider.example/CREDENTIAL");
    let shown = format!("{url}");
    assert!(!shown.contains("CREDENTIAL"), "credential leaked: {shown}");
}

// ---------------------------------------------------------------------------
// SecretKey — unconditional
// ---------------------------------------------------------------------------

#[test]
fn secret_key_display_is_masked() {
    let key = SecretKey::new("abc123");
    assert_eq!(format!("{key}"), "****");
}

#[test]
fn secret_key_debug_is_masked() {
    let key = SecretKey::new("abc123");
    assert_eq!(format!("{key:?}"), "SecretKey(****)");
}

#[test]
fn secret_key_expose_returns_raw() {
    let raw = "abc123";
    let key = SecretKey::new(raw);
    assert_eq!(key.expose(), raw);
}

/// The regression this type exists to prevent: a value with neither `?` nor
/// `@` is exactly what `SecretUrl` returned in the clear. `SecretKey` knows
/// nothing about shape, so there is no shape that gets through it.
#[test]
fn secret_key_masks_values_of_every_shape() {
    for raw in [
        "a-bare-api-key",
        "https://host/?api-key=abc",
        "postgresql://u:p@h/db",
        "",
        "?",
        "@",
    ] {
        let key = SecretKey::new(raw);
        assert_eq!(format!("{key}"), "****", "leaked for input {raw:?}");
        assert_eq!(format!("{key:?}"), "SecretKey(****)", "leaked for {raw:?}");
    }
}
