use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{Protocol, VestingUnlockCursor, VestingUnlockOrder, VestingUnlockPage, Window},
};

use super::rows::VestingUnlockRow;
use crate::repositories::helper::{PageBuilder, QueryMode};

/// The page of `rows`, read as `order` over `window`; `None` before the first reading.
pub(super) fn page_of(
    rows: Vec<VestingUnlockRow>,
    protocol: Protocol,
    window: Window,
    order: VestingUnlockOrder,
    had_cursor: bool,
    limit: i64,
) -> RepositoryResult<Option<VestingUnlockPage>> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let Some(as_of) = first.as_of else {
        return Ok(None);
    };
    let covered_since = first.covered_since;
    let keyed = rows
        .into_iter()
        .filter_map(|row| row.into_unlock(protocol).transpose())
        .collect::<RepositoryResult<Vec<_>>>()?;
    let page =
        PageBuilder::new(keyed, limit, QueryMode::Forward, had_cursor).finalize(|(unlock, key)| {
            Cursor::VestingUnlock(Box::new(VestingUnlockCursor {
                protocol,
                as_of,
                window,
                order,
                amount_usd: *key,
                unlocks_at: unlock.unlocks_at,
                pool_address: unlock.pool_address,
                position: unlock.position,
            }))
        });
    // ⚠️ Forward only: a previous cursor would be read forward from the first item.
    Ok(Some(VestingUnlockPage {
        as_of,
        covered_since,
        page: Page {
            items: page.items.into_iter().map(|(unlock, _)| unlock).collect(),
            next_cursor: page.next_cursor,
            prev_cursor: None,
            is_first: page.is_first,
            is_last: page.is_last,
        },
    }))
}
