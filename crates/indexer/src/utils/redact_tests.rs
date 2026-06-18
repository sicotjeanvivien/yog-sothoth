use super::*;

#[test]
fn redact_single_api_key_in_url() {
    let input = "https://example.com/?api-key=abc123";
    assert_eq!(
        redact_api_key(input),
        "https://example.com/?api-key=***REDACTED***"
    );
}

#[test]
fn redact_api_key_followed_by_ampersand() {
    let input = "https://example.com/?api-key=abc123&foo=bar";
    assert_eq!(
        redact_api_key(input),
        "https://example.com/?api-key=***REDACTED***&foo=bar"
    );
}

#[test]
fn redact_api_key_inside_parentheses() {
    let input = "HTTP error for url (https://example.com/?api-key=abc123)";
    assert_eq!(
        redact_api_key(input),
        "HTTP error for url (https://example.com/?api-key=***REDACTED***)"
    );
}

#[test]
fn redact_api_key_idempotent() {
    let already_redacted = "https://example.com/?api-key=***REDACTED***";
    assert_eq!(redact_api_key(already_redacted), already_redacted);
}

#[test]
fn redact_no_api_key_unchanged() {
    let input = "no secret here";
    assert_eq!(redact_api_key(input), input);
}

#[test]
fn redact_multiple_api_keys() {
    let input = "first url ?api-key=aaa and second ?api-key=bbb done";
    assert_eq!(
        redact_api_key(input),
        "first url ?api-key=***REDACTED*** and second ?api-key=***REDACTED*** done"
    );
}
