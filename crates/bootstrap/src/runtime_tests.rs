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

// ── The stop stays audible ───────────────────────────────────────────────────

/// ⚠️ **The case that was measured, and the reason this guard is code rather
/// than a comment.** This repository's own `RUST_LOG` is six per-crate
/// directives with no bare level; after the stop moved to `yog_bootstrap`, it
/// printed not one line of a shutdown — including the `warn!` naming a stage
/// destroyed mid-write. Ten cycles came back silent and looked exactly like a
/// correction that had done nothing.
#[test]
fn a_directive_only_filter_cannot_silence_the_stop() {
    let filter = build_filter(Some("yog_indexer=debug,yog_context=debug")).to_string();

    assert!(
        filter.contains("yog_bootstrap=info"),
        "the stop must stay audible, got: {filter}"
    );
    assert!(
        filter.contains("yog_indexer=debug") && filter.contains("yog_context=debug"),
        "and nothing the operator asked for may be dropped, got: {filter}"
    );
}

/// The two ways an operator has already said what they want, and neither is
/// second-guessed. A bare level anywhere covers every target; naming
/// `yog_bootstrap` covers it explicitly.
#[test]
fn a_filter_that_already_covers_the_target_is_left_alone() {
    // Compared against the same value re-parsed rather than against the raw
    // string: `EnvFilter`'s own `Display` reorders directives, so an equality
    // on the input would fail for a reason that has nothing to do with the
    // guard. What must hold is that nothing was *added*.
    for raw in ["info,sqlx=warn", "yog_indexer=debug,warn"] {
        let filter = build_filter(Some(raw)).to_string();
        assert!(
            !filter.contains("yog_bootstrap"),
            "a bare level already covers every target, got: {filter}"
        );
        assert_eq!(filter, EnvFilter::new(raw).to_string(), "{raw}");
    }
}

/// ⚠️ **Including silencing it on purpose.** An operator who writes
/// `yog_bootstrap=off` means it, and a guard that overrode them would be a
/// worse defect than the one it fixes: a filter that ignores what it is told.
#[test]
fn naming_the_target_wins_even_to_silence_it() {
    let filter = build_filter(Some("yog_bootstrap=off,yog_api=debug")).to_string();

    assert!(
        !filter.contains("yog_bootstrap=info"),
        "an explicit directive must not be overridden, got: {filter}"
    );
}
