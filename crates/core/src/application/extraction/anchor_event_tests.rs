use super::*;

/// Helper: build a minimal valid event_cpi byte sequence.
fn build_event_bytes(disc: [u8; DISCRIMINATOR_LEN], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN + payload.len());
    out.extend_from_slice(&EVENT_IX_TAG);
    out.extend_from_slice(&disc);
    out.extend_from_slice(payload);
    out
}

#[test]
fn decodes_well_formed_event() {
    let disc = [9u8, 8, 7, 6, 5, 4, 3, 2];
    let payload = [42u8, 43, 44];
    let bytes = build_event_bytes(disc, &payload);

    let (got_disc, got_payload) = decode_anchor_event_cpi(&bytes).unwrap();
    assert_eq!(got_disc, disc);
    assert_eq!(got_payload, payload);
}

#[test]
fn decodes_event_with_empty_payload() {
    let disc = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let bytes = build_event_bytes(disc, &[]);

    let (got_disc, got_payload) = decode_anchor_event_cpi(&bytes).unwrap();
    assert_eq!(got_disc, disc);
    assert!(got_payload.is_empty());
}

/// Mutation: testing the length before the tag, which makes these a decoding failure.
#[test]
fn short_untagged_data_is_not_an_event() {
    // 8 bytes: an argument-less instruction, a bare discriminator; 9: one `u8` argument.
    for bytes in [
        vec![0xb4, 0x26, 0x9a, 0x11, 0x85, 0x21, 0xa2, 0xd3],
        vec![0xb4, 0x26, 0x9a, 0x11, 0x85, 0x21, 0xa2, 0xd3, 1],
    ] {
        let err = decode_anchor_event_cpi(&bytes).unwrap_err();
        assert!(
            matches!(err, AnchorDecodeError::NotAnAnchorEvent),
            "{} bytes: {err:?}",
            bytes.len()
        );
    }
}

/// Mutation: dropping the length check that runs before the tag, which makes these ordinary
/// instructions, skipped without being counted.
#[test]
fn data_shorter_than_any_instruction_is_too_short() {
    for bytes in [vec![], vec![1u8, 2, 3], EVENT_IX_TAG[..7].to_vec()] {
        let err = decode_anchor_event_cpi(&bytes).unwrap_err();
        assert!(
            matches!(err, AnchorDecodeError::PayloadTooShort { min: 8, got } if got == bytes.len()),
            "{} bytes: {err:?}",
            bytes.len()
        );
    }
}

/// Mutation: treating anything below 16 bytes as an ordinary instruction.
#[test]
fn tagged_but_truncated_payload_is_too_short() {
    for (bytes, len) in [
        (EVENT_IX_TAG.to_vec(), 8),
        ([&EVENT_IX_TAG[..], &[1, 2, 3]].concat(), 11),
    ] {
        let err = decode_anchor_event_cpi(&bytes).unwrap_err();
        assert!(
            matches!(err, AnchorDecodeError::PayloadTooShort { min: 16, got } if got == len),
            "{len} bytes: {err:?}"
        );
    }
}

#[test]
fn rejects_payload_with_wrong_tag() {
    // 16 bytes of zeros — long enough, but tag is wrong.
    let bytes = vec![0u8; 16];
    let err = decode_anchor_event_cpi(&bytes).unwrap_err();
    assert!(matches!(err, AnchorDecodeError::NotAnAnchorEvent));
}

#[test]
fn tag_constant_matches_documented_value() {
    // Mutation: any edit of the tag, which would make every Anchor event unparseable.
    assert_eq!(
        EVENT_IX_TAG,
        [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]
    );
}
