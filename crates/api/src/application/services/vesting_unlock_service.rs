//! A protocol's vesting releases to come, as of its last reading.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        Protocol, VestingUnlockCursor, VestingUnlockLookup, VestingUnlockOrder, VestingUnlockPage,
        Window,
    },
};

/// A page of the releases, or a cursor taken in a reading replaced since.
pub(crate) enum VestingUnlockRead {
    /// `None` before the first reading.
    Page(Option<Box<VestingUnlockPage>>),
    CursorExpired,
}

pub(crate) struct VestingUnlockService {
    vesting_unlocks: Arc<dyn VestingUnlockLookup>,
}

impl VestingUnlockService {
    pub(crate) fn new(vesting_unlocks: Arc<dyn VestingUnlockLookup>) -> Self {
        Self { vesting_unlocks }
    }

    /// ⚠️ A cursor names its reading: once replaced, its keys list another window.
    pub(crate) async fn list(
        &self,
        protocol: Protocol,
        window: Window,
        order: VestingUnlockOrder,
        cursor: Option<VestingUnlockCursor>,
        limit: i64,
    ) -> RepositoryResult<VestingUnlockRead> {
        let taken_at = cursor.as_ref().map(|c| c.as_of);
        let read = self
            .vesting_unlocks
            .list(protocol, window, order, cursor, limit)
            .await?;
        Ok(match (taken_at, &read) {
            (Some(at), Some(page)) if page.as_of != at => VestingUnlockRead::CursorExpired,
            (Some(_), None) => VestingUnlockRead::CursorExpired,
            _ => VestingUnlockRead::Page(read.map(Box::new)),
        })
    }
}

#[cfg(test)]
#[path = "tests/vesting_unlock_service_tests.rs"]
mod tests;
