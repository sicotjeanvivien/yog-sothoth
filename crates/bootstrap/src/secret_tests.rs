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

// ---------------------------------------------------------------------------
// SecretUrl — the fragment, and a userinfo that is itself the credential
// ---------------------------------------------------------------------------

/// `#` was recognised as a delimiter by two of the three rules and hidden by
/// none, so a credential parked there came back whole. It is the gap between an
/// invariant stated — "the places a URL hides a credential" — and one held.
#[test]
fn fragment_is_redacted() {
    for raw in [
        "https://rpc.example.com/#api-key=abc123",
        "https://host/path#SECRET",
        "wss://host#SECRET",
    ] {
        let shown = format!("{}", SecretUrl::new(raw));
        assert!(
            !shown.contains("abc123") && !shown.contains("SECRET"),
            "fragment leaked: {shown} (from {raw})"
        );
    }
}

/// A query string and a fragment together: the query rule truncates first, so
/// the fragment is already gone. Asserted so the ordering is not accidental.
#[test]
fn query_and_fragment_together_leave_nothing() {
    let url = SecretUrl::new("https://host/?api-key=abc123#SECRET");
    assert_eq!(format!("{url}"), "https://host/?***REDACTED***");
}

/// `https://<token>@host` is a real authentication shape, so a userinfo with no
/// password is not automatically a harmless username. The host survives — only
/// the credential goes.
#[test]
fn bare_userinfo_is_a_secret_outside_postgres() {
    let url = SecretUrl::new("https://SECRETTOKEN@rpc.example.com/");
    let shown = format!("{url}");
    assert!(!shown.contains("SECRETTOKEN"), "token leaked: {shown}");
    assert!(
        shown.contains("rpc.example.com"),
        "host lost for nothing: {shown}"
    );
}

/// Postgres is the exception, and for a reason that is ours: a userinfo without
/// a password is one of the five least-privilege role names, and naming the role
/// is precisely the diagnostic this type exists to keep.
#[test]
fn bare_userinfo_in_postgres_is_a_role_name_and_survives() {
    let url = SecretUrl::new("postgresql://yog@localhost:5433/yog_sothoth");
    assert_eq!(
        format!("{url}"),
        "postgresql://yog@localhost:5433/yog_sothoth"
    );
}

// ---------------------------------------------------------------------------
// SecretUrl::scrub — a string somebody else built
// ---------------------------------------------------------------------------

/// The message shape this exists for, measured on 4 September 2026 by driving
/// `RpcClient` at an unresolvable host: reqwest renders the whole URL, and
/// `solana-client` passes it straight through.
fn reqwest_style_error(url: &str) -> String {
    format!("error sending request for url ({url})")
}

#[test]
fn scrub_removes_a_credential_from_a_third_party_error() {
    let url = SecretUrl::new("https://solana-mainnet.g.alchemy.com/v2/SUPERSECRETKEY");
    let scrubbed = url.scrub(&reqwest_style_error(url.expose()));
    assert!(
        !scrubbed.contains("SUPERSECRETKEY"),
        "credential survived: {scrubbed}"
    );
    assert!(
        scrubbed.contains("solana-mainnet.g.alchemy.com"),
        "host lost — the error no longer says what failed: {scrubbed}"
    );
}

/// The carrier is why the whole URL is replaced by its redacted form rather
/// than by the placeholder: an error that no longer names the endpoint costs
/// more than it saves.
#[test]
fn scrub_keeps_the_sentence_and_the_carrier() {
    let url = SecretUrl::new("postgresql://yog_indexer:hunter2@localhost:5433/yog_sothoth");
    let scrubbed = url.scrub(&format!("pool timed out for {}", url.expose()));
    assert_eq!(
        scrubbed,
        "pool timed out for postgresql://yog_indexer:***REDACTED***@localhost:5433/yog_sothoth"
    );
}

/// ⚠️ The binding between [`SecretUrl::scrub`] and the redaction rules.
///
/// `scrub` derives what to remove from `redact`, so the day `redact` learns a
/// new hiding place `scrub` learns it too. If someone re-implements
/// `secret_parts` by re-parsing the URL, this is what turns red.
///
/// **The haystack drops the scheme on purpose.** With the URL present verbatim,
/// the whole-value replacement removes the secret on its own and this test
/// passes no matter what `secret_parts` returns — which is exactly what it did
/// in its first shape: a mutation that made `secret_parts` return only the
/// query string left it green. Stripping the scheme is what forces the
/// per-part pass to be the thing under test.
#[test]
fn scrub_removes_every_secret_that_display_hides() {
    let cases = [
        (
            "postgresql://yog_indexer:hunter2@localhost:5433/db",
            "hunter2",
        ),
        ("https://mainnet.helius-rpc.com/?api-key=abc123", "abc123"),
        ("https://solana-mainnet.g.alchemy.com/v2/PATHKEY", "PATHKEY"),
        ("wss://xxx.quiknode.pro/TOKEN123456/", "TOKEN123456"),
        ("https://rpc.example.com/#FRAGMENTSECRET", "FRAGMENTSECRET"),
        ("https://TOKENASUSER@rpc.example.com/", "TOKENASUSER"),
        ("postgresql://yog:pa/ss@localhost:5433/db", "pa/ss"),
        ("postgresql://yog:pa#ss@localhost:5433/db", "pa#ss"),
        // Two hiding places at once: a re-parse that handles one and forgets
        // the other passes every single-secret case above.
        (
            "https://host/v2/PATHSECRET?api-key=QUERYSECRET",
            "PATHSECRET",
        ),
        (
            "https://host/v2/PATHSECRET?api-key=QUERYSECRET",
            "QUERYSECRET",
        ),
    ];
    for (raw, secret) in cases {
        let url = SecretUrl::new(raw);
        assert!(
            !format!("{url}").contains(secret),
            "Display leaked {secret} for {raw}"
        );

        let without_scheme = &raw[raw.find("://").expect("test URLs have a scheme") + 3..];
        let scrubbed = url.scrub(&format!("connect failed for {without_scheme}"));
        assert!(
            !scrubbed.contains(secret),
            "scrub left {secret} in {scrubbed} (from {raw})"
        );
    }
}

/// A third party may reformat the URL — a normalised trailing slash is the
/// common one — so the whole-value match misses. The per-part pass is what
/// catches it, and this is the test that keeps that pass alive.
#[test]
fn scrub_still_removes_the_secret_from_a_reformatted_url() {
    let url = SecretUrl::new("https://rpc.example.com?api-key=SECRET123");
    let reformatted = "error sending request for url (https://rpc.example.com/?api-key=SECRET123)";
    let scrubbed = url.scrub(reformatted);
    assert!(
        !scrubbed.contains("SECRET123"),
        "reformatted URL escaped the scrub: {scrubbed}"
    );
}

/// A message that never mentioned the URL comes back untouched — the scrub must
/// not mangle text it has no business in.
#[test]
fn scrub_leaves_an_unrelated_message_alone() {
    let url = SecretUrl::new("https://rpc.example.com/?api-key=abc123");
    let msg = "transaction not found after retries";
    assert_eq!(url.scrub(msg), msg);
}

/// When the userinfo is ambiguous the whole value is the secret, and `scrub`
/// inherits that from `redact` rather than restating it.
#[test]
fn scrub_of_an_ambiguous_url_leaves_nothing_of_it() {
    let url = SecretUrl::new("postgresql://yog:pa/ss@localhost:5433/yog_sothoth");
    let scrubbed = url.scrub(&reqwest_style_error(url.expose()));
    for fragment in ["pa/ss", "localhost", "yog_sothoth"] {
        assert!(
            !scrubbed.contains(fragment),
            "`{fragment}` survived an ambiguous URL: {scrubbed}"
        );
    }
    assert!(scrubbed.starts_with("error sending request for url (postgresql://"));
}
