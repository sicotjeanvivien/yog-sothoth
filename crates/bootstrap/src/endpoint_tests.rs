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
use crate::{ConfigError, env::required_endpoint};
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
    Endpoint::new(template.to_string(), key.map(SecretKey::new))
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
