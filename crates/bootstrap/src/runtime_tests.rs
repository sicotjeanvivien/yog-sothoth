use super::*;

/// Mutation: an untrimmed `RUST_LOG`, where `info\r` is a target matching nothing.
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
    // Against a vacuous pass: both sides could match while meaning nothing.
    assert_eq!(build_filter(Some("info")).to_string(), "info");
}

#[test]
fn absent_blank_and_unparseable_all_fall_back_to_info() {
    for raw in [None, Some(""), Some("   \r\n"), Some("=,=,=")] {
        assert_eq!(build_filter(raw).to_string(), "info", "{raw:?}");
    }
}

// ── The stop stays audible ───────────────────────────────────────────────────

/// Mutation: no guard, and the stop's lines are silenced.
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

/// Mutation: testing "has no `=`" instead of parsing a `LevelFilter`.
#[test]
fn neither_a_bare_target_nor_an_empty_segment_counts_as_a_global_level() {
    for raw in ["yog_indexer,yog_context", "yog_api=debug,"] {
        let filter = build_filter(Some(raw)).to_string();
        assert!(
            filter.contains("yog_bootstrap=info"),
            "{raw:?} ne porte aucun niveau global — le stop doit rester audible, obtenu : {filter}"
        );
    }
}

#[test]
fn a_bare_shutdown_target_covers_itself() {
    let filter = build_filter(Some("yog_bootstrap,yog_api=debug")).to_string();

    assert!(
        !filter.contains("yog_bootstrap=info"),
        "une cible nommée, même sans niveau, a déjà tranché : {filter}"
    );
    assert!(filter.contains("yog_bootstrap=trace"), "{filter}");
}

#[test]
fn a_filter_that_already_covers_the_target_is_left_alone() {
    // ⚠️ Compared to the re-parsed value: `EnvFilter`'s `Display` reorders directives.
    for raw in ["info,sqlx=warn", "yog_indexer=debug,warn"] {
        let filter = build_filter(Some(raw)).to_string();
        assert!(
            !filter.contains("yog_bootstrap"),
            "a bare level already covers every target, got: {filter}"
        );
        assert_eq!(filter, EnvFilter::new(raw).to_string(), "{raw}");
    }
}

/// Mutation: overriding an explicit `yog_bootstrap=off`.
#[test]
fn naming_the_target_wins_even_to_silence_it() {
    let filter = build_filter(Some("yog_bootstrap=off,yog_api=debug")).to_string();

    assert!(
        !filter.contains("yog_bootstrap=info"),
        "an explicit directive must not be overridden, got: {filter}"
    );
}
