//! How a credential's random bytes are written as text: lowercase hex, the one form a key or a
//! ticket is parsed back from.

pub(super) fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}
