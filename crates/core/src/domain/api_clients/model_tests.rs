use super::*;

const PREFIX: [u8; API_KEY_PREFIX_BYTES] = [1, 2, 3, 4];

fn secret() -> [u8; API_KEY_SECRET_BYTES] {
    std::array::from_fn(|i| i as u8)
}

const TEXT: &str = "yog_01020304_000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

/// Mutation: the hex of one part dropped, reordered or in uppercase.
#[test]
fn a_key_is_written_in_lowercase_hex_after_its_marker() {
    let key = ApiKey::from_random(PREFIX, secret());

    assert_eq!(key.expose(), TEXT);
    assert_eq!(key.prefix(), "01020304");
}

/// Mutation: `parse` keeping anything but the text it was given.
#[test]
fn a_written_key_parses_back_to_itself() {
    assert_eq!(
        ApiKey::parse(TEXT),
        Ok(ApiKey::from_random(PREFIX, secret()))
    );
}

/// Mutation: the hash taken over the secret alone, or over anything but the whole key.
#[test]
fn the_hash_is_the_sha256_of_the_whole_key() {
    let hash = ApiKey::parse(TEXT).expect("a valid key").hash();

    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "f5f04886b5c93a1af333b99f976c7d75312499b808ec417fe4f74ebee7b6311c"
    );
}

/// Mutation: `Debug` derived, which would print the secret into every log line.
#[test]
fn debug_shows_the_prefix_and_never_the_secret() {
    let shown = format!("{:?}", ApiKey::from_random(PREFIX, secret()));

    assert_eq!(shown, "ApiKey(yog_01020304_…)");
}

// One refusal per rule, each named by its reason.

#[test]
fn a_key_without_its_marker_is_refused() {
    assert_eq!(
        ApiKey::parse(&TEXT.replacen("yog_", "gol_", 1)),
        Err(ApiKeyFormatError::Marker)
    );
}

#[test]
fn a_key_without_its_separator_is_refused() {
    let joined = TEXT.replacen("01020304_", "01020304", 1);
    assert_eq!(ApiKey::parse(&joined), Err(ApiKeyFormatError::Parts));
}

#[test]
fn a_key_with_a_short_prefix_is_refused() {
    let short = TEXT.replacen("01020304", "010203", 1);
    assert_eq!(ApiKey::parse(&short), Err(ApiKeyFormatError::PrefixLength));
}

#[test]
fn a_key_with_a_short_secret_is_refused() {
    assert_eq!(
        ApiKey::parse(&TEXT[..TEXT.len() - 2]),
        Err(ApiKeyFormatError::SecretLength)
    );
}

#[test]
fn a_key_in_uppercase_hex_is_refused() {
    assert_eq!(
        ApiKey::parse(&TEXT.to_uppercase().replacen("YOG_", "yog_", 1)),
        Err(ApiKeyFormatError::NotLowerHex)
    );
}

#[test]
fn a_key_with_a_non_hex_character_is_refused() {
    let tampered = format!("{}g", &TEXT[..TEXT.len() - 1]);
    assert_eq!(
        ApiKey::parse(&tampered),
        Err(ApiKeyFormatError::NotLowerHex)
    );
}

// The client's name, one rule per case.

/// Mutation: a rule of `is_client_name` dropped.
#[test]
fn a_client_name_is_lowercase_letters_digits_and_dashes() {
    assert!(is_client_name("dashboard"));
    assert!(is_client_name("acme-2"));
    assert!(is_client_name(&"a".repeat(63)));
}

#[test]
fn an_empty_client_name_is_refused() {
    assert!(!is_client_name(""));
}

#[test]
fn a_client_name_past_63_characters_is_refused() {
    assert!(!is_client_name(&"a".repeat(64)));
}

#[test]
fn a_client_name_starting_with_a_dash_is_refused() {
    assert!(!is_client_name("-acme"));
}

#[test]
fn a_client_name_with_an_uppercase_letter_or_a_space_is_refused() {
    assert!(!is_client_name("Acme"));
    assert!(!is_client_name("acme corp"));
}
