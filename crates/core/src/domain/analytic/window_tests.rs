use super::*;

/// Against a label that no longer matches what `window=` and migration 029 take.
#[test]
fn the_labels_are_those_of_the_contract() {
    let labels: Vec<_> = Window::ALL.iter().map(|w| w.as_str()).collect();
    assert_eq!(labels, ["24h", "7d", "30d"]);
    assert_eq!(Window::Month.duration(), chrono::TimeDelta::days(30));
}
