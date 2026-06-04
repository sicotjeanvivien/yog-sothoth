use yog_core::{PageDirection, PagePosition};

/// Internal traversal mode resolved from `(cursor, direction, position)`.
///
/// `Forward` walks the list from newer to older events (natural
/// display order); `Backward` walks the opposite way and the
/// repository reverses the result before returning so the caller
/// always sees rows in display order.
#[derive(Debug, Clone, Copy)]
pub(crate) enum QueryMode {
    Forward,
    Backward,
}

pub(crate) fn resolve_query_mode<C>(
    position: Option<PagePosition>,
    cursor: &Option<C>,
    direction: PageDirection,
) -> QueryMode {
    match (position, cursor) {
        (Some(PagePosition::First), _) => QueryMode::Forward,
        (Some(PagePosition::Last), _) => QueryMode::Backward,
        (None, None) => QueryMode::Forward,
        (None, Some(_)) => match direction {
            PageDirection::Next => QueryMode::Forward,
            PageDirection::Prev => QueryMode::Backward,
        },
    }
}

#[cfg(test)]
#[path = "tests/query_mod_tests.rs"]
mod tests;
