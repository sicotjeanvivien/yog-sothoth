use super::*;
use crate::domain::{ApiKey, ApiKeyFormatError, TicketFormatError};

fn secret() -> [u8; TICKET_SECRET_BYTES] {
    std::array::from_fn(|i| i as u8)
}

const TEXT: &str = "yogt_000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

/// Mutation: the hex dropped, reordered or in uppercase, or another marker.
#[test]
fn a_ticket_is_written_in_lowercase_hex_after_its_marker() {
    assert_eq!(Ticket::from_random(secret()).expose(), TEXT);
}

/// Mutation: `parse` keeping anything but the text it was given.
#[test]
fn a_written_ticket_parses_back_to_itself() {
    assert_eq!(Ticket::parse(TEXT), Ok(Ticket::from_random(secret())));
}

/// Mutation: the hash taken over the secret alone, or over anything but the whole ticket.
#[test]
fn the_hash_is_the_sha256_of_the_whole_ticket() {
    let hash = Ticket::parse(TEXT).expect("a valid ticket").hash();

    let expected: [u8; 32] = Sha256::digest(TEXT.as_bytes()).into();
    assert_eq!(hash, expected);
    assert_ne!(
        hash,
        <[u8; 32]>::from(Sha256::digest(&TEXT.as_bytes()[5..]))
    );
}

/// Mutation: `Debug` derived, which would print the secret into every log line.
#[test]
fn debug_never_shows_the_secret() {
    assert_eq!(
        format!("{:?}", Ticket::from_random(secret())),
        "Ticket(yogt_…)"
    );
}

/// Mutation: a ticket's marker read as a key's, or the reverse: one could pass for the other.
#[test]
fn a_ticket_and_a_key_never_parse_as_each_other() {
    let key = ApiKey::from_random([1, 2, 3, 4], secret());

    assert_eq!(Ticket::parse(key.expose()), Err(TicketFormatError::Marker));
    assert_eq!(ApiKey::parse(TEXT), Err(ApiKeyFormatError::Marker));
}

// One refusal per rule, each named by its reason.

#[test]
fn a_ticket_without_its_marker_is_refused() {
    assert_eq!(
        Ticket::parse(&TEXT.replacen("yogt_", "tgoy_", 1)),
        Err(TicketFormatError::Marker)
    );
}

#[test]
fn a_ticket_with_a_short_secret_is_refused() {
    assert_eq!(
        Ticket::parse(&TEXT[..TEXT.len() - 2]),
        Err(TicketFormatError::SecretLength)
    );
}

#[test]
fn a_ticket_with_a_long_secret_is_refused() {
    assert_eq!(
        Ticket::parse(&format!("{TEXT}00")),
        Err(TicketFormatError::SecretLength)
    );
}

#[test]
fn a_ticket_in_uppercase_hex_is_refused() {
    assert_eq!(
        Ticket::parse(&TEXT.to_uppercase().replacen("YOGT_", "yogt_", 1)),
        Err(TicketFormatError::NotLowerHex)
    );
}

#[test]
fn a_ticket_with_a_non_hex_character_is_refused() {
    let tampered = format!("{}g", &TEXT[..TEXT.len() - 1]);
    assert_eq!(
        Ticket::parse(&tampered),
        Err(TicketFormatError::NotLowerHex)
    );
}
