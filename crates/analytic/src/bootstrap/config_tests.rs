use super::*;

/// Against either bound dropped: zero panics in `tokio::time::interval`, and
/// past `MAX_HOURS_PER_CYCLE` hours the writer never catches up.
#[test]
fn the_cycle_is_refused_outside_one_second_to_one_hour() {
    for refused in [0, MAX_CYCLE_SECS + 1] {
        let err = cycle(refused).unwrap_err();
        assert!(err.to_string().contains("ANALYTIC_CYCLE_SECS"), "{err}");
    }
    assert_eq!(cycle(1).unwrap(), Duration::from_secs(1));
    assert_eq!(
        cycle(MAX_CYCLE_SECS).unwrap(),
        Duration::from_secs(MAX_CYCLE_SECS)
    );
}
