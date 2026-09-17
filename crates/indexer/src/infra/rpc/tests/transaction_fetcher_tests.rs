use super::*;

#[test]
fn rate_limit_variants_are_classified() {
    for raw in [
        "HTTP 429 Too Many Requests",
        "rate limit exceeded",
        "Too Many Requests",
    ] {
        let e = FetchError::from_rpc_string(raw.to_string());
        assert!(matches!(e, FetchError::RateLimited), "raw = {raw:?}");
        assert_eq!(e.metric_label(), "rate_limited");
    }
}

#[test]
fn timeout_is_classified() {
    let e = FetchError::from_rpc_string("request timed out".to_string());
    assert!(matches!(e, FetchError::Timeout));
    assert_eq!(e.metric_label(), "timeout");
}

#[test]
fn null_response_maps_to_not_found() {
    let e = FetchError::from_rpc_string("got null in response".to_string());
    assert!(matches!(e, FetchError::NotFound));
    assert_eq!(e.metric_label(), "not_found");
}

#[test]
fn unknown_falls_back_to_other() {
    let e = FetchError::from_rpc_string("some unexpected RPC error".to_string());
    assert!(matches!(e, FetchError::Other(_)));
    assert_eq!(e.metric_label(), "other");
}

#[test]
fn connection_keyword_is_classified() {
    let e = FetchError::from_rpc_string("connection refused".to_string());
    assert!(matches!(e, FetchError::Connection(_)));
    assert_eq!(e.metric_label(), "connection_error");
}

// ── the version ceiling ─────────────────────────────────────────────

/// The refusal exactly as the provider wrote it on 15 September 2026, for the
/// signature `2yyZaQ3w…` that is now `swap_v1.json`.
const VERSION_REFUSAL: &str = "RPC response error -32015: Transaction version (1) is not \
     supported by the requesting client. Please try the request again with the following \
     configuration parameter: \"maxSupportedTransactionVersion\": 1";

#[test]
fn a_version_refusal_is_classified_as_such() {
    let e = FetchError::from_rpc_string(VERSION_REFUSAL.to_string());
    assert!(matches!(e, FetchError::UnsupportedVersion(_)), "got {e:?}");
    assert_eq!(e.metric_label(), "unsupported_version");
}

#[test]
fn a_version_refusal_is_not_retried() {
    let e = FetchError::from_rpc_string(VERSION_REFUSAL.to_string());
    assert!(
        !e.is_retryable(),
        "a version refusal is deterministic — retrying it spends five calls for nothing"
    );
}

#[test]
fn every_other_failure_is_retried() {
    for e in [
        FetchError::NotFound,
        FetchError::RateLimited,
        FetchError::Timeout,
        FetchError::Connection("connection refused".to_string()),
        FetchError::Other("some unexpected RPC error".to_string()),
    ] {
        assert!(e.is_retryable(), "{e:?} must still be retried");
    }
}

/// ⚠️ **The ceiling equals the highest version the corpus proves the adapter
/// reads — not less, not more.** Below it, `getTransaction` refuses a
/// transaction the adapter could have read, and that transaction is lost.
/// Above it, the RPC hands over a format no fixture has shown the adapter
/// reads correctly, and a wrong reading is silent. Both directions are
/// defects, so the test is an equality.
#[test]
fn the_declared_ceiling_is_the_highest_fixture_version() {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../core/tests/fixtures/damm_v2");
    let mut highest: Option<(u8, String)> = None;

    for entry in std::fs::read_dir(&dir).expect("the fixture directory exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let raw = std::fs::read_to_string(&path).expect("readable fixture");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("fixture is JSON");
        // `"legacy"` carries no number, and no ceiling refuses it.
        let Some(version) = json["version"].as_u64() else {
            continue;
        };
        let version = u8::try_from(version).expect("a transaction version fits a u8");

        assert!(
            version <= MAX_SUPPORTED_TRANSACTION_VERSION,
            "{name} is version {version}, above the declared ceiling \
             {MAX_SUPPORTED_TRANSACTION_VERSION}: getTransaction would refuse it with -32015 \
             and the transaction would be lost"
        );
        if highest.as_ref().is_none_or(|(h, _)| version > *h) {
            highest = Some((version, name));
        }
    }

    let (highest, name) = highest.expect("the corpus holds versioned transactions");
    assert_eq!(
        MAX_SUPPORTED_TRANSACTION_VERSION, highest,
        "the ceiling claims version {MAX_SUPPORTED_TRANSACTION_VERSION}, but the highest \
         version any fixture proves the adapter reads is {highest} ({name}) — add a mainnet \
         fixture of the new version before raising it"
    );
}
