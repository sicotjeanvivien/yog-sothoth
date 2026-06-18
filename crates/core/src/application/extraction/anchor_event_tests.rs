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

#[test]
fn rejects_payload_below_minimum_size() {
    // Just a few bytes — not even enough for the tag.
    let bytes = vec![1u8, 2, 3];
    let err = decode_anchor_event_cpi(&bytes).unwrap_err();
    assert!(matches!(
        err,
        AnchorDecodeError::PayloadTooShort { got: 3, .. }
    ));
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
    // EVENT_IX_TAG_LE must equal the bytes 1d 9a cb 51 2e a5 45 e4 —
    // this is sha256("anchor:event")[..8] in little-endian.
    // If this assertion ever fails, every Anchor program's events
    // would be unparseable; this guards against accidental edits.
    assert_eq!(
        EVENT_IX_TAG,
        [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]
    );
}
