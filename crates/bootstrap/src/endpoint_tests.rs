//! Tests for [`Endpoint`] and its reader, `required_endpoint`.
//!
//! Every guard below was put in failure before being trusted: the assembly, the
//! two refusals and the fail-closed `Display` were each mutated in turn and
//! each turned this file red. A guard nobody has seen fail is a guard nobody
//! knows works.
//!
//! SAFETY-NOTE on env tests, as in `env_tests.rs`: `set_var` is process-global
//! and the harness is parallel, so every test here uses **unique key names**.

use super::*;
use crate::{
    ConfigError,
    env::{required_endpoint, required_endpoint_with_header},
};
use std::env;

/// Build one without an environment to read it from.
///
/// The crate-private constructor, **not** `Endpoint::for_tests`, even though
/// that exists for other crates: a feature-gated constructor used here would
/// compile green under `--workspace` — where `yog-context`'s dev-dependency
/// turns `test-support` on for everyone — and red under
/// `cargo test -p yog-bootstrap`. Measured on this file, which is the trap
/// `crates/README.md` warns about, seen from inside.
fn endpoint(template: &str, key: Option<&str>) -> Endpoint {
    Endpoint::new(template.to_string(), None, key.map(SecretKey::new))
}

/// The same, for an endpoint whose credential rides in a header.
fn with_header(template: &str, header: (&str, &str), key: Option<&str>) -> Endpoint {
    Endpoint::new(
        template.to_string(),
        Some((header.0.to_string(), header.1.to_string())),
        key.map(SecretKey::new),
    )
}

// ── assembly ────────────────────────────────────────────────────────

/// The substitution happens where the operator wrote it, and nowhere else —
/// which is the whole reason the code knows no provider: Helius' `?api-key=`
/// and Alchemy's `/v2/<key>` are the same line of code here.
#[test]
fn the_key_is_substituted_at_the_placeholder() {
    let query = endpoint("https://host/?api-key={key}", Some("abc123"));
    assert_eq!(query.url().expose(), "https://host/?api-key=abc123");

    let path = endpoint("https://host/v2/{key}", Some("abc123"));
    assert_eq!(path.url().expose(), "https://host/v2/abc123");
}

/// A public endpoint is called exactly as written. `api.mainnet-beta.solana.com`
/// wants no credential, and the local workflow points all four endpoints at it.
#[test]
fn a_url_without_a_placeholder_is_used_verbatim() {
    let public = endpoint("https://api.mainnet-beta.solana.com", None);
    assert_eq!(public.url().expose(), "https://api.mainnet-beta.solana.com");
}

// ── what the reader refuses ─────────────────────────────────────────

/// The refusal names **the variable**, not just its kind: the operator reads
/// this in a crash log with nothing else to go on, and `MissingVariable` is the
/// one variant that carries no value alongside it.
#[test]
fn a_placeholder_without_its_key_is_refused_by_name() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_EP_NOKEY_URL", "https://host/?api-key={key}");
        env::remove_var("YOG_TEST_EP_NOKEY_KEY");
    }
    match required_endpoint("YOG_TEST_EP_NOKEY") {
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, "YOG_TEST_EP_NOKEY_KEY"),
        other => panic!("expected MissingVariable(YOG_TEST_EP_NOKEY_KEY), got {other:?}"),
    }
}

/// A blank `_KEY` is the same oversight as an absent one, and must not be the
/// gap the guard above leaves open: `FOO=` in a `.env` — or `${FOO:-}` in the
/// compose file, which is how the container gets it — must not start a process
/// whose address carries a literal `{key}`.
#[test]
fn a_blank_key_is_refused_like_an_absent_one() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_EP_BLANK_URL", "https://host/?api-key={key}");
        env::set_var("YOG_TEST_EP_BLANK_KEY", "  \r\n");
    }
    match required_endpoint("YOG_TEST_EP_BLANK") {
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, "YOG_TEST_EP_BLANK_KEY"),
        other => panic!("expected MissingVariable(YOG_TEST_EP_BLANK_KEY), got {other:?}"),
    }
}

/// The same failure from the other side: a credential that is configured and
/// goes nowhere. Both variables are named, because the fix is in one of them
/// and the operator is the one who knows which.
#[test]
fn a_key_with_nowhere_to_go_is_refused_naming_both() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_EP_ORPHAN_URL", "https://host/rpc");
        env::set_var("YOG_TEST_EP_ORPHAN_KEY", "abc123");
    }
    match required_endpoint("YOG_TEST_EP_ORPHAN") {
        Err(ConfigError::UnsupportedCombination { detail }) => {
            assert!(detail.contains("YOG_TEST_EP_ORPHAN_KEY"), "{detail}");
            assert!(detail.contains("YOG_TEST_EP_ORPHAN_URL"), "{detail}");
            assert!(
                !detail.contains("abc123"),
                "the refusal put the credential in the message: {detail}"
            );
        }
        other => panic!("expected UnsupportedCombination, got {other:?}"),
    }
}

/// The URL itself is required, and its absence names it — the reader must not
/// swallow it into the key's refusal.
#[test]
fn an_absent_url_is_refused_by_name() {
    match required_endpoint("YOG_TEST_EP_ABSENT") {
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, "YOG_TEST_EP_ABSENT_URL"),
        other => panic!("expected MissingVariable(YOG_TEST_EP_ABSENT_URL), got {other:?}"),
    }
}

/// The two names are **derived from the prefix**. Asserted here because that
/// derivation is the point of taking a prefix rather than two names: a caller
/// spelling them itself is a convention, and this repository has measured what
/// conventions are worth at the ninth site.
#[test]
fn both_variables_are_derived_from_the_prefix() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_EP_PREFIX_URL", "https://host/v2/{key}");
        env::set_var("YOG_TEST_EP_PREFIX_KEY", "abc123");
    }
    let endpoint = required_endpoint("YOG_TEST_EP_PREFIX").expect("both halves are set");
    assert_eq!(endpoint.url().expose(), "https://host/v2/abc123");
}

/// `required` trims for the whole workspace; the pair inherits it, on both
/// halves. The `.env` here is CRLF and the native workflow sources it.
#[test]
fn both_halves_are_trimmed() {
    // SAFETY: unique keys, isolated from other tests
    unsafe {
        env::set_var("YOG_TEST_EP_CRLF_URL", "https://host/v2/{key}\r\n");
        env::set_var("YOG_TEST_EP_CRLF_KEY", " abc123\r");
    }
    let endpoint = required_endpoint("YOG_TEST_EP_CRLF").expect("both halves are set");
    assert_eq!(endpoint.url().expose(), "https://host/v2/abc123");
}

// ── what it prints ──────────────────────────────────────────────────

/// The return on the whole ticket: an address with `{key}` in it hides nothing,
/// so a startup log names the endpoint **in full** — no arbitrage left between
/// the diagnostic and the leak.
#[test]
fn a_template_is_printed_whole() {
    let endpoint = endpoint(
        "https://mainnet.helius-rpc.com/?api-key={key}",
        Some("s3cret"),
    );
    assert_eq!(
        endpoint.to_string(),
        "https://mainnet.helius-rpc.com/?api-key={key}"
    );
    assert!(
        !format!("{endpoint:?}").contains("s3cret"),
        "Debug leaked the key: {endpoint:?}"
    );
}

/// And the fail-closed half. An operator who pastes the credential straight
/// into the URL — the old shape, which every existing `.env` still has — gets
/// the redacted form, because nothing here can tell that URL apart from a
/// genuinely public one. Without this, the readability win of the test above
/// would double as a leak the first time somebody filled the file the old way.
#[test]
fn a_url_without_a_placeholder_is_redacted() {
    let pasted = endpoint("https://mainnet.helius-rpc.com/?api-key=s3cret", None);
    assert_eq!(
        pasted.to_string(),
        "https://mainnet.helius-rpc.com/?***REDACTED***"
    );

    let in_path = endpoint("https://solana-mainnet.g.alchemy.com/v2/s3cret", None);
    assert_eq!(
        in_path.to_string(),
        "https://solana-mainnet.g.alchemy.com/***REDACTED***"
    );
}

/// A `{key}` accounts for the credential the operator moved out — and for
/// nothing else. The components a template has no business carrying are
/// redacted whether or not it has a placeholder, because "it has a `{key}`, so
/// it hides nothing" is an assumption about the *rest* of the URL that nobody
/// can make. Found in review, 6 September 2026: this line printed whole.
#[test]
fn a_second_credential_does_not_ride_the_placeholder_out() {
    let userinfo = endpoint("https://user:s3cret@host/?api-key={key}", Some("k"));
    assert_eq!(
        userinfo.to_string(),
        "https://user:***REDACTED***@host/?api-key={key}"
    );
    assert!(
        !format!("{userinfo:?}").contains("s3cret"),
        "Debug leaked it: {userinfo:?}"
    );

    // A bare-token userinfo — `https://<token>@host` — has no `:` to split on,
    // so the whole of it goes; the host is what the log needed anyway.
    let token = endpoint("https://s3cret@host/v2/{key}", Some("k"));
    assert_eq!(token.to_string(), "https://***REDACTED***@host/v2/{key}");

    let fragment = endpoint("https://host/v2/{key}#s3cret", Some("k"));
    assert_eq!(fragment.to_string(), "https://host/v2/{key}#***REDACTED***");
}

/// And the readability the fix must not cost: path and query are exactly where
/// the placeholder lives, so they stay legible. A redaction that swallowed them
/// would give back the arbitrage this whole ticket removed.
#[test]
fn the_placeholder_and_its_carrier_stay_legible() {
    let query = endpoint("https://mainnet.helius-rpc.com/?api-key={key}", Some("k"));
    assert_eq!(
        query.to_string(),
        "https://mainnet.helius-rpc.com/?api-key={key}"
    );

    let path = endpoint("https://solana-mainnet.g.alchemy.com/v2/{key}", Some("k"));
    assert_eq!(
        path.to_string(),
        "https://solana-mainnet.g.alchemy.com/v2/{key}"
    );
}

// ── the four shapes, one test each ──────────────────────────────────

/// Shape 1 — the credential rides in a header, the URL carries none.
/// QuickNode, Alchemy, Shyft and Helius all authenticate a gRPC stream this
/// way, and it is the shape `Endpoint` could not express before.
#[test]
fn a_credential_in_a_header_is_substituted_there() {
    let endpoint = with_header(
        "https://example.solana-mainnet.quiknode.pro:443",
        ("x-token", "{key}"),
        Some("s3cret"),
    );

    assert_eq!(
        endpoint.url().expose(),
        "https://example.solana-mainnet.quiknode.pro:443",
        "the URL must come back untouched — it never held the credential"
    );
    let (name, value) = endpoint.header().expect("the header is configured");
    assert_eq!(name, "x-token");
    assert_eq!(value.expose(), "s3cret");
}

/// Shape 2 — the credential rides in the URL, as it already did. Kept as a
/// test of its own so that the header work cannot quietly regress it: Triton's
/// load balancers take `user:password` basic auth, so this is a live shape and
/// not merely the legacy one.
#[test]
fn a_credential_in_the_url_still_works_with_no_header() {
    let basic = endpoint("https://token_user:{key}@host:443", Some("s3cret"));

    assert_eq!(basic.url().expose(), "https://token_user:s3cret@host:443");
    assert!(
        basic.header().is_none(),
        "no header was configured, so none must be produced"
    );
}

/// Shape 3 — no credential at all. A self-hosted Yellowstone ships
/// `"x_token": null`, and a public JSON-RPC node wants nothing either.
#[test]
fn an_endpoint_with_no_credential_has_neither_key_nor_header() {
    let public = endpoint("https://api.mainnet-beta.solana.com", None);

    assert_eq!(public.url().expose(), "https://api.mainnet-beta.solana.com");
    assert!(public.header().is_none());
}

/// Shape 4 — **the one that proves the header name is not carved in.** If
/// anything in this crate knew the string `x-token`, this test would be the one
/// to fail, which is why it is written with a different scheme entirely.
#[test]
fn the_header_name_is_whatever_the_operator_wrote() {
    let bearer = with_header(
        "https://host",
        ("authorization", "Bearer {key}"),
        Some("s3cret"),
    );

    let (name, value) = bearer.header().expect("the header is configured");
    assert_eq!(name, "authorization");
    assert_eq!(
        value.expose(),
        "Bearer s3cret",
        "the placeholder is substituted inside the value, not instead of it"
    );
}

// ── the truth table, read on two carriers ───────────────────────────

/// The generalisation itself: a `{key}` living **only** in the header is a
/// `{key}`, so the `_KEY` beside it is accepted rather than refused as going
/// nowhere. Restricting the search back to the URL turns this red — which is
/// the mutation that proves the rule was widened and not duplicated.
#[test]
fn a_placeholder_in_the_header_alone_accepts_its_key() {
    unsafe {
        env::set_var("HDR_ONLY_URL", "https://host:443");
        env::set_var("HDR_ONLY_HEADER_NAME", "x-token");
        env::set_var("HDR_ONLY_HEADER_VALUE", "{key}");
        env::set_var("HDR_ONLY_KEY", "s3cret");
    }

    let endpoint = required_endpoint_with_header("HDR_ONLY").expect("a key with somewhere to go");
    let (name, value) = endpoint.header().expect("the header is configured");
    assert_eq!(name, "x-token");
    assert_eq!(value.expose(), "s3cret");

    unsafe {
        env::remove_var("HDR_ONLY_URL");
        env::remove_var("HDR_ONLY_HEADER_NAME");
        env::remove_var("HDR_ONLY_HEADER_VALUE");
        env::remove_var("HDR_ONLY_KEY");
    }
}

/// And the other side of the same rule: with a header that carries no
/// placeholder either, the key really does go nowhere, and the refusal must
/// name **both** carriers so the operator knows where a `{key}` may go.
#[test]
fn a_key_with_no_placeholder_in_either_carrier_is_refused() {
    unsafe {
        env::set_var("HDR_NOWHERE_URL", "https://host:443");
        env::set_var("HDR_NOWHERE_HEADER_NAME", "x-region");
        env::set_var("HDR_NOWHERE_HEADER_VALUE", "eu-west");
        env::set_var("HDR_NOWHERE_KEY", "s3cret");
    }

    let error = required_endpoint_with_header("HDR_NOWHERE").expect_err("the key goes nowhere");
    let ConfigError::UnsupportedCombination { detail } = error else {
        panic!("expected UnsupportedCombination, got {error:?}");
    };
    assert!(detail.contains("HDR_NOWHERE_URL"), "{detail}");
    assert!(detail.contains("HDR_NOWHERE_HEADER_VALUE"), "{detail}");
    assert!(detail.contains("HDR_NOWHERE_KEY"), "{detail}");
    assert!(
        !detail.contains("s3cret"),
        "the refusal repeated the credential: {detail}"
    );

    unsafe {
        env::remove_var("HDR_NOWHERE_URL");
        env::remove_var("HDR_NOWHERE_HEADER_NAME");
        env::remove_var("HDR_NOWHERE_HEADER_VALUE");
        env::remove_var("HDR_NOWHERE_KEY");
    }
}

/// Half a pair stops the process at startup, names **both** variables, and —
/// the part that matters — never repeats the value, which is what carries the
/// credential. `ConfigError::InvalidValue` has a `value` field that would put it
/// in the crash log; this path must never reach that variant.
#[test]
fn half_a_header_pair_is_refused_naming_both_variables() {
    for (suffix, name, value, because) in [
        (
            "NO_VALUE",
            Some("x-token"),
            None,
            "without `HDR_NO_VALUE_HEADER_VALUE`",
        ),
        (
            "NO_NAME",
            None,
            Some("s3cretpasted"),
            "without `HDR_NO_NAME_HEADER_NAME`",
        ),
    ] {
        let prefix = format!("HDR_{suffix}");
        unsafe {
            env::set_var(format!("{prefix}_URL"), "https://host:443");
            if let Some(name) = name {
                env::set_var(format!("{prefix}_HEADER_NAME"), name);
            }
            if let Some(value) = value {
                env::set_var(format!("{prefix}_HEADER_VALUE"), value);
            }
        }

        let error = required_endpoint_with_header(&prefix).expect_err("half a pair");
        let ConfigError::UnsupportedCombination { detail } = error else {
            panic!("{suffix}: expected UnsupportedCombination, got {error:?}");
        };
        assert!(
            detail.contains(because),
            "{suffix}: wanted {because:?}, got: {detail}"
        );
        assert!(
            !detail.contains("s3cretpasted"),
            "{suffix}: the refusal echoed the value: {detail}"
        );

        unsafe {
            env::remove_var(format!("{prefix}_URL"));
            env::remove_var(format!("{prefix}_HEADER_NAME"));
            env::remove_var(format!("{prefix}_HEADER_VALUE"));
        }
    }
}

/// ⚠️ **With no header configured, the two doors produce the same endpoint** —
/// which is what makes `_with_header` safe for a variable that *may* carry one
/// rather than one that must.
///
/// Raised as a doubt in review of PR #138 on 10 September 2026: `yog-indexer`
/// reads `INGEST_STREAM` through `_with_header` unconditionally, and the
/// question was whether that imposes a header on an endpoint that has none. It
/// does not. The door decides which **configurations are refused**, never what
/// a header-less one yields: same URL, same absence of header, same printed
/// form. Only when a pair is actually set do the two part ways, and that is
/// [`a_header_on_a_url_only_consumer_is_refused`].
#[test]
fn without_a_header_the_two_doors_produce_the_same_endpoint() {
    // A credentialed endpoint, key in the URL.
    unsafe {
        env::set_var("HDR_NONE_URL", "https://host:443/v2/{key}");
        env::set_var("HDR_NONE_KEY", "s3cret");
    }

    let plain = required_endpoint("HDR_NONE").expect("a key with somewhere to go");
    let with_door = required_endpoint_with_header("HDR_NONE").expect("the same, other door");

    assert_eq!(plain.url().expose(), with_door.url().expose());
    assert!(plain.header().is_none() && with_door.header().is_none());
    assert_eq!(
        format!("{plain}"),
        format!("{with_door}"),
        "even what they print is the same — the door is not part of the endpoint"
    );

    // And a public one, where there is no credential at all.
    unsafe {
        env::remove_var("HDR_NONE_KEY");
        env::set_var("HDR_NONE_URL", "https://api.mainnet-beta.solana.com");
    }

    let plain = required_endpoint("HDR_NONE").expect("a public endpoint");
    let with_door = required_endpoint_with_header("HDR_NONE").expect("still public");

    assert_eq!(plain.url().expose(), with_door.url().expose());
    assert!(plain.header().is_none() && with_door.header().is_none());

    unsafe {
        env::remove_var("HDR_NONE_URL");
    }
}

/// A blank half is an absent half, and the two must agree on what "set" means —
/// `optional` decides it once, here it is inherited rather than restated.
#[test]
fn a_blank_half_counts_as_absent_not_as_present() {
    unsafe {
        env::set_var("HDR_BLANK_URL", "https://host:443");
        env::set_var("HDR_BLANK_HEADER_NAME", "   ");
        env::set_var("HDR_BLANK_HEADER_VALUE", "   ");
    }

    let endpoint = required_endpoint_with_header("HDR_BLANK").expect("both halves are blank");
    assert!(
        endpoint.header().is_none(),
        "a blank pair must read as no header at all"
    );

    unsafe {
        env::remove_var("HDR_BLANK_URL");
        env::remove_var("HDR_BLANK_HEADER_NAME");
        env::remove_var("HDR_BLANK_HEADER_VALUE");
    }
}

// ── what the header prints ──────────────────────────────────────────

/// The header half of the fail-closed rule. A value carrying `{key}` prints
/// whole — that is the diagnostic an operator needs, and it hides nothing. A
/// value without one is a credential somebody inlined, or a header that needs
/// none, and nothing here can tell them apart.
#[test]
fn a_header_value_is_printed_only_when_its_key_is_outside_it() {
    let templated = with_header("https://host", ("x-token", "{key}"), Some("s3cret"));
    assert_eq!(templated.to_string(), "https://host [x-token: {key}]");
    assert!(
        !format!("{templated:?}").contains("s3cret"),
        "Debug leaked the key: {templated:?}"
    );

    let pasted = with_header("https://host", ("x-token", "s3cret"), None);
    assert_eq!(pasted.to_string(), "https://host [x-token: ****]");
    assert!(
        !format!("{pasted:?}").contains("s3cret"),
        "Debug leaked the inlined credential: {pasted:?}"
    );
}

/// An endpoint with no header prints exactly as it did before this feature
/// existed. Four endpoints in production have none, and their startup lines
/// must not have moved a character.
#[test]
fn an_endpoint_without_a_header_prints_as_it_always_did() {
    let plain = endpoint("https://mainnet.helius-rpc.com/?api-key={key}", Some("k"));
    assert_eq!(
        plain.to_string(),
        "https://mainnet.helius-rpc.com/?api-key={key}",
        "no header means nothing appended — not an empty bracket"
    );
}

// ── the header only counts where somebody sends it ──────────────────

/// **The refusal this whole split exists for.** A `_HEADER` set on an endpoint
/// whose consumer sends only the URL is a credential that goes nowhere — and
/// unlike a wrong key, it fails *upward*: the process connects anonymously and
/// any endpoint tolerating anonymous callers answers normally.
///
/// Found in review, 8 September 2026. The first shape of this feature accepted
/// it, and the verification run that was supposed to prove the feature actually
/// demonstrated the defect — `# Connected.` on a stream carrying no credential.
#[test]
fn a_header_on_a_url_only_consumer_is_refused() {
    unsafe {
        env::set_var("HDR_UNREAD_URL", "wss://api.mainnet-beta.solana.com");
        env::set_var("HDR_UNREAD_HEADER_NAME", "x-token");
        env::set_var("HDR_UNREAD_HEADER_VALUE", "{key}");
        env::set_var("HDR_UNREAD_KEY", "s3cret");
    }

    let error = required_endpoint("HDR_UNREAD").expect_err("nothing would send this header");
    let ConfigError::UnsupportedCombination { detail } = error else {
        panic!("expected UnsupportedCombination, got {error:?}");
    };
    assert!(detail.contains("HDR_UNREAD_HEADER_NAME"), "{detail}");
    assert!(detail.contains("HDR_UNREAD_URL"), "{detail}");
    assert!(
        !detail.contains("s3cret"),
        "the refusal repeated the credential: {detail}"
    );

    // And the same variables, read by a consumer that does send it, are fine —
    // which is what makes the refusal a statement about the *caller* rather
    // than a ban on headers.
    let ok = required_endpoint_with_header("HDR_UNREAD").expect("this consumer sends it");
    let (name, value) = ok.header().expect("the header is configured");
    assert_eq!(name, "x-token");
    assert_eq!(value.expose(), "s3cret");

    unsafe {
        env::remove_var("HDR_UNREAD_URL");
        env::remove_var("HDR_UNREAD_HEADER_NAME");
        env::remove_var("HDR_UNREAD_HEADER_VALUE");
        env::remove_var("HDR_UNREAD_KEY");
    }
}

/// The url-only door is not merely stricter — it is the *unchanged* one. Every
/// endpoint in production reads through it, and none of them may have gained a
/// refusal from this feature.
#[test]
fn a_url_only_endpoint_is_unaffected_by_the_feature() {
    unsafe {
        env::set_var("HDR_PLAIN_URL", "https://host/?api-key={key}");
        env::set_var("HDR_PLAIN_KEY", "s3cret");
    }

    let endpoint = required_endpoint("HDR_PLAIN").expect("the shape that already worked");
    assert_eq!(endpoint.url().expose(), "https://host/?api-key=s3cret");
    assert!(endpoint.header().is_none());

    unsafe {
        env::remove_var("HDR_PLAIN_URL");
        env::remove_var("HDR_PLAIN_KEY");
    }
}

// ── refusals must not point at each other ───────────────────────────

/// A refusal that names a fix the *next* refusal undoes is worse than one that
/// names none. Found in review, 8 September 2026: the advice offered the header
/// as a destination on an endpoint that refuses headers, so an operator who
/// followed it earned the opposite refusal on the next start.
#[test]
fn a_refusal_never_advises_what_the_next_refusal_forbids() {
    // A key with nowhere to go, on a url-only endpoint. The advice must not
    // offer the header as a destination — writing one there is refused.
    unsafe {
        env::set_var("HDR_ADVICE_URL", "https://host");
        env::set_var("HDR_ADVICE_KEY", "s3cret");
    }
    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint("HDR_ADVICE").expect_err("the key goes nowhere")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(
        !detail.contains("HDR_ADVICE_HEADER_VALUE") && !detail.contains("in the header"),
        "advised a header on an endpoint that refuses one: {detail}"
    );
    // The same endpoint read by a header-aware consumer *may* say it.
    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint_with_header("HDR_ADVICE").expect_err("still nowhere to go")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(detail.contains("header"), "{detail}");
    unsafe {
        env::remove_var("HDR_ADVICE_URL");
        env::remove_var("HDR_ADVICE_KEY");
    }
}

/// A name is not a header name if it carries a space or a `:`. That is shape,
/// not charset — and the `:` case is the migration hazard: this feature shipped
/// once as a single `<PREFIX>_HEADER=name: value`, so pasting the whole of it
/// into `_HEADER_NAME` is the mistake an operator actually makes. Left accepted
/// it would also **print in the clear**, a name never being masked.
///
/// The single-variable parser refused whitespace and the split dropped it;
/// review caught the regression on 8 September 2026.
#[test]
fn a_header_name_carrying_a_space_or_a_colon_is_refused() {
    // ⚠️ `PASTED_TIGHT` has no space, and it is the only case that observes the
    // `:` rule on its own: found by mutation, 8 September 2026 — with only the
    // spaced variants, removing the `:` check left every case still refused by
    // the whitespace rule, so one of the two rules was never tested.
    for (suffix, name) in [
        ("SPACED", "x token"),
        ("PASTED", "x-token: s3cretpasted"),
        ("PASTED_TIGHT", "x-token:s3cretpasted"),
    ] {
        let prefix = format!("HDR_NAME_{suffix}");
        unsafe {
            env::set_var(format!("{prefix}_URL"), "https://host:443");
            env::set_var(format!("{prefix}_HEADER_NAME"), name);
            env::set_var(format!("{prefix}_HEADER_VALUE"), "{key}");
            env::set_var(format!("{prefix}_KEY"), "s3cret");
        }

        let error = required_endpoint_with_header(&prefix).expect_err("not a header name");
        let ConfigError::UnsupportedCombination { detail } = error else {
            panic!("{suffix}: expected UnsupportedCombination, got {error:?}");
        };
        assert!(
            detail.contains(&format!("{prefix}_HEADER_NAME")),
            "{detail}"
        );
        assert!(
            !detail.contains("s3cretpasted"),
            "{suffix}: the refusal echoed the pasted credential: {detail}"
        );

        unsafe {
            env::remove_var(format!("{prefix}_URL"));
            env::remove_var(format!("{prefix}_HEADER_NAME"));
            env::remove_var(format!("{prefix}_HEADER_VALUE"));
            env::remove_var(format!("{prefix}_KEY"));
        }
    }
}

/// And what the check must not cost: a name with surrounding whitespace is
/// trimmed by `optional` and accepted, since that is a `.env` artefact and not
/// an operator's meaning.
#[test]
fn a_header_name_is_trimmed_not_refused() {
    unsafe {
        env::set_var("HDR_TRIM_URL", "https://host:443");
        env::set_var("HDR_TRIM_HEADER_NAME", "  x-token  ");
        env::set_var("HDR_TRIM_HEADER_VALUE", "  {key}  ");
        env::set_var("HDR_TRIM_KEY", "s3cret");
    }

    let endpoint = required_endpoint_with_header("HDR_TRIM").expect("trimmed, not refused");
    let (name, value) = endpoint.header().expect("the header is configured");
    assert_eq!(name, "x-token");
    assert_eq!(value.expose(), "s3cret");

    unsafe {
        env::remove_var("HDR_TRIM_URL");
        env::remove_var("HDR_TRIM_HEADER_NAME");
        env::remove_var("HDR_TRIM_HEADER_VALUE");
        env::remove_var("HDR_TRIM_KEY");
    }
}

/// Three refusals can fire on the same input, and **which one fires first is
/// the message**. Both orderings below were verified by mutation on
/// 8 September 2026, and the second one was silent until this test existed:
/// moving the `header_is_read` refusal after the pairing match left all 80
/// tests green, though its own comment claims the order is load-bearing.
#[test]
fn the_first_refusal_is_the_one_that_ends_the_matter() {
    // 1. A bad name with no value yet. Shape must answer, not pairing: an
    //    operator told to complete the pair would earn the shape refusal on the
    //    next start — the defect this whole family of tests exists to catch.
    unsafe {
        env::set_var("HDR_SEQ_A_URL", "https://host:443");
        env::set_var("HDR_SEQ_A_HEADER_NAME", "x-token: s3cretpasted");
    }
    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint_with_header("HDR_SEQ_A").expect_err("not a header name")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(
        detail.contains("is not a header name"),
        "pairing answered before shape: {detail}"
    );
    assert!(
        !detail.contains("s3cretpasted"),
        "the refusal echoed the pasted credential: {detail}"
    );
    unsafe {
        env::remove_var("HDR_SEQ_A_URL");
        env::remove_var("HDR_SEQ_A_HEADER_NAME");
    }

    // 2. Half a pair on a url-only consumer. "Nobody would send it" must answer
    //    before "complete the pair", or the operator completes a pair they are
    //    then told to delete.
    unsafe {
        env::set_var("HDR_SEQ_B_URL", "https://host:443");
        env::set_var("HDR_SEQ_B_HEADER_NAME", "x-token");
    }
    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint("HDR_SEQ_B").expect_err("nothing would send it")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(
        detail.contains("sends only the URL"),
        "pairing answered before the gate: {detail}"
    );
    assert!(
        !detail.contains("Set both"),
        "advised completing a pair that is then refused: {detail}"
    );
    unsafe {
        env::remove_var("HDR_SEQ_B_URL");
        env::remove_var("HDR_SEQ_B_HEADER_NAME");
    }
}

/// A `{key}` in the header NAME is never substituted — it would reach the
/// client as the literal name `{key}`. The dangerous half is the second case:
/// with a placeholder in the URL as well, the config used to be **accepted**.
#[test]
fn a_placeholder_in_the_header_name_is_refused_even_when_the_url_has_one() {
    unsafe {
        env::set_var("HDR_NAMEKEY_URL", "https://host/?api-key={key}");
        env::set_var("HDR_NAMEKEY_HEADER_NAME", "{key}");
        env::set_var("HDR_NAMEKEY_HEADER_VALUE", "jeton");
        env::set_var("HDR_NAMEKEY_KEY", "s3cret");
    }

    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint_with_header("HDR_NAMEKEY").expect_err("the name is not a carrier")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(detail.contains("HDR_NAMEKEY_HEADER_NAME"), "{detail}");
    // Not `contains("NAME")`, which the line above already satisfies through
    // the variable name — it asserted nothing. What must be said is *where* the
    // placeholder belongs, since that is the fix the operator needs.
    assert!(detail.contains("HDR_NAMEKEY_HEADER_VALUE"), "{detail}");

    unsafe {
        env::remove_var("HDR_NAMEKEY_URL");
        env::remove_var("HDR_NAMEKEY_HEADER_NAME");
        env::remove_var("HDR_NAMEKEY_HEADER_VALUE");
        env::remove_var("HDR_NAMEKEY_KEY");
    }
}
