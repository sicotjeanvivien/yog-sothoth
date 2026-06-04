use crate::repositories::helper::QueryMode;
use yog_core::{Cursor, Page};

pub(crate) struct PageBuilder<T> {
    items: Vec<T>,
    effective_limit: i64,
    mode: QueryMode,
    had_cursor: bool,
}

impl<T> PageBuilder<T> {
    pub(crate) fn new(
        items: Vec<T>,
        effective_limit: i64,
        mode: QueryMode,
        had_cursor: bool,
    ) -> Self {
        Self {
            items,
            effective_limit,
            mode,
            had_cursor,
        }
    }

    pub(crate) fn finalize(mut self, make_cursor: impl Fn(&T) -> Cursor) -> Page<T> {
        let has_more = self.items.len() as i64 > self.effective_limit;

        if has_more {
            self.items.truncate(self.effective_limit as usize);
        }

        if matches!(self.mode, QueryMode::Backward) {
            self.items.reverse();
        }

        let (is_first, is_last) = match self.mode {
            QueryMode::Forward => (!self.had_cursor, !has_more),
            QueryMode::Backward => (!has_more, !self.had_cursor),
        };

        let (prev_cursor, next_cursor) =
            build_page_cursors(&self.items, is_first, is_last, make_cursor);

        Page {
            items: self.items,
            prev_cursor,
            next_cursor,
            is_first,
            is_last,
        }
    }
}

fn build_page_cursors<T>(
    items: &[T],
    is_first: bool,
    is_last: bool,
    make_cursor: impl Fn(&T) -> Cursor,
) -> (Option<Cursor>, Option<Cursor>) {
    if items.is_empty() {
        return (None, None);
    }

    let prev = if is_first {
        None
    } else {
        items.first().map(&make_cursor)
    };

    let next = if is_last {
        None
    } else {
        items.last().map(&make_cursor)
    };

    (prev, next)
}

#[cfg(test)]
#[path = "tests/pagination_tests.rs"]
mod tests;
