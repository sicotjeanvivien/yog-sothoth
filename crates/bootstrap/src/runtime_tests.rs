use super::*;

/// The regression this exists for: `RUST_LOG=info` with a trailing `\r` —
/// the CRLF `.env` sourced into the shell — used to reach `EnvFilter`
/// untrimmed, where `info\r` parses as a *target name* rather than a level.
/// The result was not a fallback to `info`: it was a filter matching
/// nothing, i.e. a daemon that silently stopped logging. Measured on the
/// indexer binary, 2 September 2026: one startup line with `info`, zero
/// with `info\r`.
#[test]
fn a_trailing_carriage_return_does_not_change_the_filter() {
    assert_eq!(
        build_filter(Some("info\r\n")).to_string(),
        build_filter(Some("info")).to_string(),
    );
    assert_eq!(
        build_filter(Some(" info,sqlx=warn\r")).to_string(),
        build_filter(Some("info,sqlx=warn")).to_string(),
    );
}

#[test]
fn a_bare_level_survives_intact() {
    // Guards the assertion above against being vacuously true: if trimming
    // ever mangled the value into something else, both sides would still
    // match each other while meaning nothing.
    assert_eq!(build_filter(Some("info")).to_string(), "info");
}

#[test]
fn absent_blank_and_unparseable_all_fall_back_to_info() {
    for raw in [None, Some(""), Some("   \r\n"), Some("=,=,=")] {
        assert_eq!(build_filter(raw).to_string(), "info", "{raw:?}");
    }
}
