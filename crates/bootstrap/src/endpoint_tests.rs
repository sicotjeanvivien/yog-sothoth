//! Tests for [`Endpoint`] and its reader, `required_endpoint`.
//! ⚠️ `set_var` is process-global and the harness parallel: every test uses unique key names.

use super::*;
use crate::{
    ConfigError,
    env::{required_endpoint, required_endpoint_allowing_header},
};
use std::env;

/// Build one without an environment to read it from.
/// ⚠️ Not `Endpoint::for_tests`: feature-gated, it compiles under `--workspace`
/// only, and `cargo test -p yog-bootstrap` would fail.
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

#[test]
fn the_key_is_substituted_at_the_placeholder() {
    let query = endpoint("https://host/?api-key={key}", Some("abc123"));
    assert_eq!(query.url().expose(), "https://host/?api-key=abc123");

    let path = endpoint("https://host/v2/{key}", Some("abc123"));
    assert_eq!(path.url().expose(), "https://host/v2/abc123");
}

#[test]
fn a_url_without_a_placeholder_is_used_verbatim() {
    let public = endpoint("https://api.mainnet-beta.solana.com", None);
    assert_eq!(public.url().expose(), "https://api.mainnet-beta.solana.com");
}

// ── what the reader refuses ─────────────────────────────────────────

/// Mutation: a refusal that does not name the missing `_KEY` variable.
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

/// Mutation: a blank `_KEY` read as set, which sends a literal `{key}`.
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

/// Mutation: a missing URL reported as the key's refusal.
#[test]
fn an_absent_url_is_refused_by_name() {
    match required_endpoint("YOG_TEST_EP_ABSENT") {
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, "YOG_TEST_EP_ABSENT_URL"),
        other => panic!("expected MissingVariable(YOG_TEST_EP_ABSENT_URL), got {other:?}"),
    }
}

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

/// Mutation: printing a URL without `{key}` in full; it may hold an inlined key.
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

/// Mutation: trusting a `{key}` template whole; its userinfo and fragment still go.
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

    // A bare-token userinfo has no `:` to split on: the whole of it goes.
    let token = endpoint("https://s3cret@host/v2/{key}", Some("k"));
    assert_eq!(token.to_string(), "https://***REDACTED***@host/v2/{key}");

    let fragment = endpoint("https://host/v2/{key}#s3cret", Some("k"));
    assert_eq!(fragment.to_string(), "https://host/v2/{key}#***REDACTED***");
}

/// Mutation: redacting path or query of a `{key}` template, where the placeholder lives.
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

#[test]
fn a_credential_in_the_url_still_works_with_no_header() {
    let basic = endpoint("https://token_user:{key}@host:443", Some("s3cret"));

    assert_eq!(basic.url().expose(), "https://token_user:s3cret@host:443");
    assert!(
        basic.header().is_none(),
        "no header was configured, so none must be produced"
    );
}

#[test]
fn an_endpoint_with_no_credential_has_neither_key_nor_header() {
    let public = endpoint("https://api.mainnet-beta.solana.com", None);

    assert_eq!(public.url().expose(), "https://api.mainnet-beta.solana.com");
    assert!(public.header().is_none());
}

/// Mutation: anything in this crate knowing the string `x-token`.
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

/// Mutation: looking for the placeholder in the URL only.
#[test]
fn a_placeholder_in_the_header_alone_accepts_its_key() {
    unsafe {
        env::set_var("HDR_ONLY_URL", "https://host:443");
        env::set_var("HDR_ONLY_HEADER_NAME", "x-token");
        env::set_var("HDR_ONLY_HEADER_VALUE", "{key}");
        env::set_var("HDR_ONLY_KEY", "s3cret");
    }

    let endpoint =
        required_endpoint_allowing_header("HDR_ONLY").expect("a key with somewhere to go");
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

#[test]
fn a_key_with_no_placeholder_in_either_carrier_is_refused() {
    unsafe {
        env::set_var("HDR_NOWHERE_URL", "https://host:443");
        env::set_var("HDR_NOWHERE_HEADER_NAME", "x-region");
        env::set_var("HDR_NOWHERE_HEADER_VALUE", "eu-west");
        env::set_var("HDR_NOWHERE_KEY", "s3cret");
    }

    let error = required_endpoint_allowing_header("HDR_NOWHERE").expect_err("the key goes nowhere");
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

/// Mutation: an error quoting the value, which carries the credential.
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

        let error = required_endpoint_allowing_header(&prefix).expect_err("half a pair");
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

#[test]
fn without_a_header_the_two_doors_produce_the_same_endpoint() {
    // A credentialed endpoint, key in the URL.
    unsafe {
        env::set_var("HDR_NONE_URL", "https://host:443/v2/{key}");
        env::set_var("HDR_NONE_KEY", "s3cret");
    }

    let plain = required_endpoint("HDR_NONE").expect("a key with somewhere to go");
    let with_door = required_endpoint_allowing_header("HDR_NONE").expect("the same, other door");

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
    let with_door = required_endpoint_allowing_header("HDR_NONE").expect("still public");

    assert_eq!(plain.url().expose(), with_door.url().expose());
    assert!(plain.header().is_none() && with_door.header().is_none());

    unsafe {
        env::remove_var("HDR_NONE_URL");
    }
}

#[test]
fn a_blank_half_counts_as_absent_not_as_present() {
    unsafe {
        env::set_var("HDR_BLANK_URL", "https://host:443");
        env::set_var("HDR_BLANK_HEADER_NAME", "   ");
        env::set_var("HDR_BLANK_HEADER_VALUE", "   ");
    }

    let endpoint = required_endpoint_allowing_header("HDR_BLANK").expect("both halves are blank");
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

    // The same variables, read by a consumer that sends it, are accepted.
    let ok = required_endpoint_allowing_header("HDR_UNREAD").expect("this consumer sends it");
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

/// Mutation: a refusal advising the header on a door that refuses headers.
#[test]
fn a_refusal_never_advises_what_the_next_refusal_forbids() {
    // A key with nowhere to go, on a url-only endpoint: no header in the advice.
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
        required_endpoint_allowing_header("HDR_ADVICE").expect_err("still nowhere to go")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(detail.contains("header"), "{detail}");
    unsafe {
        env::remove_var("HDR_ADVICE_URL");
        env::remove_var("HDR_ADVICE_KEY");
    }
}

/// ⚠️ `x-token: <secret>` pasted into the name would print in the clear.
#[test]
fn a_header_name_carrying_a_space_or_a_colon_is_refused() {
    // ⚠️ `PASTED_TIGHT` has no space: the only case that bites on the `:` rule alone.
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

        let error = required_endpoint_allowing_header(&prefix).expect_err("not a header name");
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

#[test]
fn a_header_name_is_trimmed_not_refused() {
    unsafe {
        env::set_var("HDR_TRIM_URL", "https://host:443");
        env::set_var("HDR_TRIM_HEADER_NAME", "  x-token  ");
        env::set_var("HDR_TRIM_HEADER_VALUE", "  {key}  ");
        env::set_var("HDR_TRIM_KEY", "s3cret");
    }

    let endpoint = required_endpoint_allowing_header("HDR_TRIM").expect("trimmed, not refused");
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

/// Mutation: moving the `header_is_read` or the shape refusal after the pairing.
#[test]
fn the_first_refusal_is_the_one_that_ends_the_matter() {
    // 1. A bad name with no value yet: shape answers, not pairing.
    unsafe {
        env::set_var("HDR_SEQ_A_URL", "https://host:443");
        env::set_var("HDR_SEQ_A_HEADER_NAME", "x-token: s3cretpasted");
    }
    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint_allowing_header("HDR_SEQ_A").expect_err("not a header name")
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

    // 2. Half a pair on a url-only consumer: "nobody would send it" answers first.
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

#[test]
fn a_placeholder_in_the_header_name_is_refused_even_when_the_url_has_one() {
    unsafe {
        env::set_var("HDR_NAMEKEY_URL", "https://host/?api-key={key}");
        env::set_var("HDR_NAMEKEY_HEADER_NAME", "{key}");
        env::set_var("HDR_NAMEKEY_HEADER_VALUE", "jeton");
        env::set_var("HDR_NAMEKEY_KEY", "s3cret");
    }

    let ConfigError::UnsupportedCombination { detail } =
        required_endpoint_allowing_header("HDR_NAMEKEY").expect_err("the name is not a carrier")
    else {
        panic!("expected UnsupportedCombination");
    };
    assert!(detail.contains("HDR_NAMEKEY_HEADER_NAME"), "{detail}");
    // ⚠️ Not `contains("NAME")`, satisfied by the variable name: assert where it belongs.
    assert!(detail.contains("HDR_NAMEKEY_HEADER_VALUE"), "{detail}");

    unsafe {
        env::remove_var("HDR_NAMEKEY_URL");
        env::remove_var("HDR_NAMEKEY_HEADER_NAME");
        env::remove_var("HDR_NAMEKEY_HEADER_VALUE");
        env::remove_var("HDR_NAMEKEY_KEY");
    }
}
