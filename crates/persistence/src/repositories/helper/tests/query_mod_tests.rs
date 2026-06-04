use super::*;

#[test]
fn should_use_forward_mode_for_first_position() {
    let mode = resolve_query_mode(Some(PagePosition::First), &Some(()), PageDirection::Prev);

    assert!(matches!(mode, QueryMode::Forward));
}

#[test]
fn should_use_backward_mode_for_last_position() {
    let mode = resolve_query_mode(Some(PagePosition::Last), &Some(()), PageDirection::Next);

    assert!(matches!(mode, QueryMode::Backward));
}

#[test]
fn should_use_forward_mode_without_cursor() {
    let mode = resolve_query_mode::<()>(None, &None, PageDirection::Next);

    assert!(matches!(mode, QueryMode::Forward));
}

#[test]
fn should_use_forward_mode_for_next_cursor_navigation() {
    let mode = resolve_query_mode(None, &Some(()), PageDirection::Next);

    assert!(matches!(mode, QueryMode::Forward));
}

#[test]
fn should_use_backward_mode_for_prev_cursor_navigation() {
    let mode = resolve_query_mode(None, &Some(()), PageDirection::Prev);

    assert!(matches!(mode, QueryMode::Backward));
}
