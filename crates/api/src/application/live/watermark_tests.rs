use std::time::{Duration, Instant};

use super::{GAP_SETTLE, LiveWatermark};

/// Mutation: `floor` moved to the highest id delivered — the 2 that commits after the 3 is lost.
#[test]
fn a_hole_filled_later_is_delivered_once() {
    let now = Instant::now();
    let mut watermark = LiveWatermark::anchored(0);

    assert_eq!(watermark.fresh(vec![1, 3], |id| *id, now), vec![1, 3]);
    watermark.settle(now);
    assert_eq!(watermark.floor(), 1);

    assert_eq!(watermark.fresh(vec![2, 3], |id| *id, now), vec![2]);
    watermark.settle(now);
    assert_eq!(watermark.floor(), 3);
}

/// Mutation: a hole waited for without limit — the floor never moves past a rolled-back id.
#[test]
fn a_hole_older_than_the_settle_is_passed() {
    let start = Instant::now();
    let mut watermark = LiveWatermark::anchored(0);
    watermark.fresh(vec![1, 3], |id| *id, start);

    watermark.settle(start);
    watermark.settle(start + GAP_SETTLE - Duration::from_millis(1));
    assert_eq!(watermark.floor(), 1, "still waiting for the 2");

    watermark.settle(start + GAP_SETTLE);
    assert_eq!(watermark.floor(), 3);
}

/// Mutation: a hole dated by the oldest id delivered — the hole at 3, under a 4 read late, passed
/// on the clock of the 5 read early.
#[test]
fn a_hole_is_dated_by_the_id_read_above_it() {
    let start = Instant::now();
    let mut watermark = LiveWatermark::anchored(0);
    watermark.fresh(vec![2, 5], |id| *id, start);
    watermark.settle(start);

    let later = start + GAP_SETTLE - Duration::from_secs(1);
    watermark.fresh(vec![1, 4], |id| *id, later);
    watermark.settle(later);
    assert_eq!(watermark.floor(), 2);

    watermark.settle(start + GAP_SETTLE + Duration::from_secs(1));
    assert_eq!(
        watermark.floor(),
        2,
        "the 4 above the hole at 3 was read two seconds ago"
    );
}

/// Mutation: a hole dated when the floor reaches it — three holes seen together cost 30 s.
#[test]
fn holes_seen_together_are_passed_together() {
    let start = Instant::now();
    let mut watermark = LiveWatermark::anchored(10);
    watermark.fresh(vec![12, 14, 16], |id| *id, start);
    watermark.settle(start);

    watermark.settle(start + GAP_SETTLE);
    assert_eq!(watermark.floor(), 16);
}
