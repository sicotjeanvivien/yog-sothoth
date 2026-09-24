use super::*;

#[test]
fn an_interval_under_a_minute_is_refused_naming_the_variable() {
    for secs in [0, 1, 59] {
        let err = interval(secs).unwrap_err();
        assert!(err.to_string().contains("ARCHIVE_INTERVAL_SECS"), "{err}");
    }
}

#[test]
fn a_minute_and_the_default_are_accepted() {
    assert_eq!(interval(60).unwrap(), Duration::from_secs(60));
    assert_eq!(
        interval(DEFAULT_INTERVAL_SECS).unwrap(),
        Duration::from_secs(6 * 60 * 60)
    );
}
