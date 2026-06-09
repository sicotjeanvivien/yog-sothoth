use super::*;
use chrono::{TimeZone, Utc};
use solana_signature::Signature;
use yog_core::domain::MeteoraDammV2SwapEventCursor;

#[derive(Debug, Clone, PartialEq)]
struct Item(u32);

fn cursor(_item: &Item) -> Cursor {
    Cursor::MeteoraDammV2SwapEvent(MeteoraDammV2SwapEventCursor {
        timestamp: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        signature: Signature::from([2; 64]),
    })
}

#[test]
fn should_build_empty_page() {
    let page = PageBuilder::new(vec![], 10, QueryMode::Forward, false).finalize(cursor);

    assert!(page.items.is_empty());

    assert!(page.is_first);
    assert!(page.is_last);

    assert!(page.prev_cursor.is_none());
    assert!(page.next_cursor.is_none());
}

#[test]
fn should_build_single_page_without_cursors() {
    let page =
        PageBuilder::new(vec![Item(1), Item(2)], 10, QueryMode::Forward, false).finalize(cursor);

    assert_eq!(page.items.len(), 2);

    assert!(page.is_first);
    assert!(page.is_last);

    assert!(page.prev_cursor.is_none());
    assert!(page.next_cursor.is_none());
}

#[test]
fn should_truncate_and_create_next_cursor() {
    let page = PageBuilder::new(
        vec![Item(1), Item(2), Item(3), Item(4)],
        3,
        QueryMode::Forward,
        false,
    )
    .finalize(cursor);

    assert_eq!(page.items.len(), 3);

    assert!(page.is_first);
    assert!(!page.is_last);

    assert!(page.prev_cursor.is_none());
    assert!(page.next_cursor.is_some());
}

#[test]
fn should_build_middle_forward_page() {
    let page = PageBuilder::new(
        vec![Item(1), Item(2), Item(3), Item(4)],
        3,
        QueryMode::Forward,
        true,
    )
    .finalize(cursor);

    assert!(!page.is_first);
    assert!(!page.is_last);

    assert!(page.prev_cursor.is_some());
    assert!(page.next_cursor.is_some());
}

#[test]
fn should_build_last_forward_page() {
    let page = PageBuilder::new(vec![Item(1), Item(2), Item(3)], 3, QueryMode::Forward, true)
        .finalize(cursor);

    assert!(!page.is_first);
    assert!(page.is_last);

    assert!(page.prev_cursor.is_some());
    assert!(page.next_cursor.is_none());
}

#[test]
fn should_reverse_backward_results() {
    let page = PageBuilder::new(
        vec![Item(3), Item(2), Item(1)],
        10,
        QueryMode::Backward,
        true,
    )
    .finalize(cursor);

    assert_eq!(page.items, vec![Item(1), Item(2), Item(3),]);
}

#[test]
fn should_build_first_backward_page() {
    let page = PageBuilder::new(
        vec![Item(3), Item(2), Item(1)],
        3,
        QueryMode::Backward,
        true,
    )
    .finalize(cursor);

    assert!(page.is_first);
    assert!(!page.is_last);

    assert!(page.prev_cursor.is_none());
    assert!(page.next_cursor.is_some());
}

#[test]
fn should_build_last_backward_page() {
    let page = PageBuilder::new(
        vec![Item(4), Item(3), Item(2), Item(1)],
        3,
        QueryMode::Backward,
        false,
    )
    .finalize(cursor);

    assert!(!page.is_first);
    assert!(page.is_last);

    assert!(page.prev_cursor.is_some());
    assert!(page.next_cursor.is_none());
}
